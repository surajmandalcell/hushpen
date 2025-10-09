//! WebSocket client for real-time updates
//! 
//! This module provides a comprehensive WebSocket client for real-time communication
//! with the backend, handling connection management, subscriptions, and event dispatching.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, Mutex};
use tokio::time::{interval, timeout};
use futures_util::{StreamExt, SinkExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;
use uuid::Uuid;
use chrono::Utc;

use crate::models::websocket::*;
use crate::utils::{AppError, AppResult};

/// Event handler type for WebSocket messages
pub type EventHandler = Arc<dyn Fn(WsMessage) + Send + Sync>;

/// WebSocket client for real-time communication
#[derive(Debug)]
pub struct WebSocketClient {
    /// WebSocket configuration
    config: WebSocketConfig,
    /// Current connection state
    connection_state: Arc<RwLock<ConnectionState>>,
    /// Session ID for this connection
    session_id: Arc<RwLock<Option<String>>>,
    /// Event handlers for different message types
    event_handlers: Arc<RwLock<HashMap<String, Vec<EventHandler>>>>,
    /// Subscribed channels
    subscriptions: Arc<RwLock<Vec<SubscriptionChannel>>>,
    /// WebSocket stream
    ws_stream: Arc<Mutex<Option<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>>>,
    /// Reconnection attempt count
    reconnect_attempts: Arc<RwLock<u32>>,
    /// Last heartbeat timestamp
    last_heartbeat: Arc<RwLock<chrono::DateTime<Utc>>>,
    /// Whether the client is running
    is_running: Arc<RwLock<bool>>,
}

impl WebSocketClient {
    /// Create a new WebSocket client with the given configuration
    pub fn new(config: WebSocketConfig) -> Self {
        Self {
            config,
            connection_state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            session_id: Arc::new(RwLock::new(None)),
            event_handlers: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(Vec::new())),
            ws_stream: Arc::new(Mutex::new(None)),
            reconnect_attempts: Arc::new(RwLock::new(0)),
            last_heartbeat: Arc::new(RwLock::new(Utc::now())),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// Create a WebSocket client with default configuration
    pub fn with_default_url(url: String) -> Self {
        let config = WebSocketConfig {
            url,
            ..Default::default()
        };
        Self::new(config)
    }

    /// Connect to the WebSocket server
    pub async fn connect(&self) -> AppResult<()> {
        // Set connection state to connecting
        {
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Connecting;
        }

        // Parse WebSocket URL
        let url = Url::parse(&self.config.url)
            .map_err(|e| AppError::api_with_source("Invalid WebSocket URL", e))?;

        // Add authentication token if provided
        let mut url = url;
        if let Some(token) = &self.config.auth_token {
            url.query_pairs_mut()
                .append_pair("token", token);
        }

        // Connect with timeout
        let ws_stream = timeout(
            Duration::from_secs(self.config.connection_timeout),
            connect_async(url)
        )
        .await
        .map_err(|_| AppError::api("WebSocket connection timeout"))?
        .map_err(|e| AppError::api_with_source("Failed to connect to WebSocket", e))?.0;

        // Store the stream
        {
            let mut stream_guard = self.ws_stream.lock().await;
            *stream_guard = Some(ws_stream);
        }

        // Set connection state to connected
        {
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Connected;
        }

        // Reset reconnection attempts
        {
            let mut attempts = self.reconnect_attempts.write().await;
            *attempts = 0;
        }

        // Start the message processing loop
        self.start_message_loop().await?;

        Ok(())
    }

    /// Disconnect from the WebSocket server
    pub async fn disconnect(&self) -> AppResult<()> {
        // Set running to false
        {
            let mut running = self.is_running.write().await;
            *running = false;
        }

        // Close the WebSocket stream if it exists
        {
            let mut stream_guard = self.ws_stream.lock().await;
            if let Some(mut ws_stream) = stream_guard.take() {
                let _ = ws_stream.close(None).await;
            }
        }

        // Set connection state to disconnected
        {
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Disconnected;
        }

        // Clear session ID
        {
            let mut session_id = self.session_id.write().await;
            *session_id = None;
        }

        Ok(())
    }

    /// Get the current connection state
    pub async fn get_connection_state(&self) -> ConnectionState {
        let state = self.connection_state.read().await;
        state.clone()
    }

    /// Get the current session ID
    pub async fn get_session_id(&self) -> Option<String> {
        let session_id = self.session_id.read().await;
        session_id.clone()
    }

    /// Subscribe to a channel
    pub async fn subscribe(&self, channel: SubscriptionChannel) -> AppResult<()> {
        // Add to subscriptions
        {
            let mut subscriptions = self.subscriptions.write().await;
            if !subscriptions.contains(&channel) {
                subscriptions.push(channel.clone());
            }
        }

        // Send subscription message if connected
        if self.get_connection_state().await == ConnectionState::Connected {
            self.send_message(WsMessage::Subscribe {
                channels: vec![channel.to_string()],
            }).await?;
        }

        Ok(())
    }

    /// Unsubscribe from a channel
    pub async fn unsubscribe(&self, channel: SubscriptionChannel) -> AppResult<()> {
        // Remove from subscriptions
        {
            let mut subscriptions = self.subscriptions.write().await;
            subscriptions.retain(|c| c != &channel);
        }

        // Send unsubscription message if connected
        if self.get_connection_state().await == ConnectionState::Connected {
            self.send_message(WsMessage::Unsubscribe {
                channels: vec![channel.to_string()],
            }).await?;
        }

        Ok(())
    }

    /// Register an event handler for a specific message type
    pub async fn register_handler<F>(&self, message_type: &str, handler: F)
    where
        F: Fn(WsMessage) + Send + Sync + 'static,
    {
        let mut handlers = self.event_handlers.write().await;
        handlers
            .entry(message_type.to_string())
            .or_insert_with(Vec::new)
            .push(Arc::new(handler));
    }

    /// Send a message to the WebSocket server
    pub async fn send_message(&self, message: WsMessage) -> AppResult<()> {
        let mut stream_guard = self.ws_stream.lock().await;
        
        if let Some(ws_stream) = stream_guard.as_mut() {
            let json = serde_json::to_string(&message)
                .map_err(|e| AppError::api_with_source("Failed to serialize WebSocket message", e))?;
            
            ws_stream.send(Message::Text(json))
                .await
                .map_err(|e| AppError::api_with_source("Failed to send WebSocket message", e))?;
            
            Ok(())
        } else {
            Err(AppError::api("WebSocket is not connected"))
        }
    }

    /// Start the message processing loop
    async fn start_message_loop(&self) -> AppResult<()> {
        // Set running to true
        {
            let mut running = self.is_running.write().await;
            *running = true;
        }

        // Clone necessary references for the task
        let ws_stream = Arc::clone(&self.ws_stream);
        let connection_state = Arc::clone(&self.connection_state);
        let session_id = Arc::clone(&self.session_id);
        let event_handlers = Arc::clone(&self.event_handlers);
        let subscriptions = Arc::clone(&self.subscriptions);
        let reconnect_attempts = Arc::clone(&self.reconnect_attempts);
        let last_heartbeat = Arc::clone(&self.last_heartbeat);
        let is_running = Arc::clone(&self.is_running);
        let config = self.config.clone();

        // Spawn the message processing task
        tokio::spawn(async move {
            // Start heartbeat task
            let heartbeat_ws_stream = Arc::clone(&ws_stream);
            let heartbeat_is_running = Arc::clone(&is_running);
            let heartbeat_interval = config.heartbeat_interval;
            
            tokio::spawn(async move {
                let mut interval = interval(Duration::from_secs(heartbeat_interval));
                
                while *heartbeat_is_running.read().await {
                    interval.tick().await;
                    
                    if *connection_state.read().await == ConnectionState::Connected {
                        let ping_message = WsMessage::Ping {
                            timestamp: Utc::now(),
                        };
                        
                        if let Ok(json) = serde_json::to_string(&ping_message) {
                            let mut stream_guard = heartbeat_ws_stream.lock().await;
                            if let Some(ws_stream) = stream_guard.as_mut() {
                                let _ = ws_stream.send(Message::Text(json)).await;
                            }
                        }
                        
                        // Update last heartbeat
                        *last_heartbeat.write().await = Utc::now();
                    }
                }
            });

            // Main message processing loop
            while *is_running.read().await {
                let message = {
                    let mut stream_guard = ws_stream.lock().await;
                    if let Some(ws_stream) = stream_guard.as_mut() {
                        match timeout(Duration::from_secs(1), ws_stream.next()).await {
                            Ok(Some(Ok(msg))) => Some(msg),
                            Ok(Some(Err(e))) => {
                                eprintln!("WebSocket error: {}", e);
                                None
                            },
                            Ok(None) => {
                                // Connection closed
                                break;
                            },
                            Err(_) => {
                                // Timeout, continue
                                None
                            },
                        }
                    } else {
                        // No stream, break
                        break;
                    }
                };

                if let Some(msg) = message {
                    match msg {
                        Message::Text(text) => {
                            if let Ok(ws_message) = serde_json::from_str::<WsMessage>(&text) {
                                // Handle the message
                                Self::handle_message(
                                    ws_message,
                                    &connection_state,
                                    &session_id,
                                    &event_handlers,
                                    &subscriptions,
                                ).await;
                            }
                        },
                        Message::Binary(data) => {
                            if let Ok(text) = String::from_utf8(data) {
                                if let Ok(ws_message) = serde_json::from_str::<WsMessage>(&text) {
                                    // Handle the message
                                    Self::handle_message(
                                        ws_message,
                                        &connection_state,
                                        &session_id,
                                        &event_handlers,
                                        &subscriptions,
                                    ).await;
                                }
                            }
                        },
                        Message::Close(_) => {
                            // Connection closed
                            break;
                        },
                        Message::Ping(_) => {
                            // Respond with pong
                            let mut stream_guard = ws_stream.lock().await;
                            if let Some(ws_stream) = stream_guard.as_mut() {
                                let _ = ws_stream.send(Message::Pong(vec![])).await;
                            }
                        },
                        Message::Pong(_) => {
                            // Update last heartbeat
                            *last_heartbeat.write().await = Utc::now();
                        },
                        Message::Frame(_) => {
                            // Handle raw frame messages
                        },
                    }
                }
            }

            // Connection lost, attempt reconnection
            if *is_running.read().await {
                *connection_state.write().await = ConnectionState::Reconnecting;
                
                // Attempt reconnection
                let mut attempts = reconnect_attempts.write().await;
                *attempts += 1;
                
                if *attempts <= config.max_reconnect_attempts {
                    // Calculate delay
                    let delay = if config.exponential_backoff {
                        let base_delay = config.reconnect_delay;
                        let exponential_delay = base_delay * 2_u64.pow(*attempts - 1);
                        std::cmp::min(exponential_delay, config.max_reconnect_delay)
                    } else {
                        config.reconnect_delay
                    };
                    
                    // Wait before reconnecting
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                    
                    // Note: In a real implementation, we would call reconnect() here
                    // For now, we'll just set the state to failed
                    *connection_state.write().await = ConnectionState::Failed;
                } else {
                    // Max attempts reached
                    *connection_state.write().await = ConnectionState::Failed;
                }
            }
        });

        Ok(())
    }

    /// Handle a received WebSocket message
    async fn handle_message(
        message: WsMessage,
        connection_state: &Arc<RwLock<ConnectionState>>,
        session_id: &Arc<RwLock<Option<String>>>,
        event_handlers: &Arc<RwLock<HashMap<String, Vec<EventHandler>>>>,
        subscriptions: &Arc<RwLock<Vec<SubscriptionChannel>>>,
    ) {
        match message {
            WsMessage::Connected { session_id: sid } => {
                // Store session ID
                *session_id.write().await = Some(sid);
                
                // Set connection state to connected
                *connection_state.write().await = ConnectionState::Connected;
                
                // Resubscribe to channels
                let subs = subscriptions.read().await;
                for channel in subs.iter() {
                    // Send subscription message
                    // In a real implementation, we would send this through the WebSocket
                }
            },
            WsMessage::Disconnected { reason } => {
                // Set connection state to disconnected
                *connection_state.write().await = ConnectionState::Disconnected;
                
                // Clear session ID
                *session_id.write().await = None;
                
                eprintln!("WebSocket disconnected: {}", reason);
            },
            WsMessage::Error { error } => {
                eprintln!("WebSocket error: {}", error);
            },
            WsMessage::Pong { timestamp } => {
                // Handle pong response
                let now = Utc::now();
                let latency = now.signed_duration_since(timestamp);
                if latency.num_milliseconds() > 5000 {
                    eprintln!("High WebSocket latency: {}ms", latency.num_milliseconds());
                }
            },
            _ => {
                // Get message type as string
                let message_type = match &message {
                    WsMessage::TranscriptionStarted(_) => "TranscriptionStarted",
                    WsMessage::TranscriptionProgress(_) => "TranscriptionProgress",
                    WsMessage::TranscriptionSegment(_) => "TranscriptionSegment",
                    WsMessage::TranscriptionCompleted(_) => "TranscriptionCompleted",
                    WsMessage::TranscriptionFailed(_) => "TranscriptionFailed",
                    WsMessage::FileUploadStarted(_) => "FileUploadStarted",
                    WsMessage::FileUploadProgress(_) => "FileUploadProgress",
                    WsMessage::FileUploadCompleted(_) => "FileUploadCompleted",
                    WsMessage::FileUploadFailed(_) => "FileUploadFailed",
                    WsMessage::ModelDownloadStarted(_) => "ModelDownloadStarted",
                    WsMessage::ModelDownloadProgress(_) => "ModelDownloadProgress",
                    WsMessage::ModelDownloadCompleted(_) => "ModelDownloadCompleted",
                    WsMessage::ModelDownloadFailed(_) => "ModelDownloadFailed",
                    WsMessage::ModelLoaded(_) => "ModelLoaded",
                    WsMessage::ModelUnloaded(_) => "ModelUnloaded",
                    WsMessage::SystemStatus(_) => "SystemStatus",
                    WsMessage::ContainerStatus(_) => "ContainerStatus",
                    WsMessage::SubscriptionConfirmed { .. } => "SubscriptionConfirmed",
                    _ => "Unknown",
                };
                
                // Call registered handlers
                let handlers = event_handlers.read().await;
                if let Some(message_handlers) = handlers.get(message_type) {
                    for handler in message_handlers {
                        handler(message.clone());
                    }
                }
                
                // Also call general handlers
                if let Some(general_handlers) = handlers.get("*") {
                    for handler in general_handlers {
                        handler(message.clone());
                    }
                }
            }
        }
    }

    /// Reconnect to the WebSocket server
    pub async fn reconnect(&self) -> AppResult<()> {
        // Disconnect first
        self.disconnect().await?;
        
        // Wait a bit before reconnecting
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // Connect again
        self.connect().await
    }

    /// Check if the client is connected
    pub async fn is_connected(&self) -> bool {
        let state = self.connection_state.read().await;
        *state == ConnectionState::Connected
    }

    /// Get the number of reconnection attempts
    pub async fn get_reconnect_attempts(&self) -> u32 {
        let attempts = self.reconnect_attempts.read().await;
        *attempts
    }

    /// Get the time since the last heartbeat
    pub async fn time_since_last_heartbeat(&self) -> chrono::Duration<Utc> {
        let last_heartbeat = self.last_heartbeat.read().await;
        Utc::now() - *last_heartbeat
    }
}

impl Drop for WebSocketClient {
    fn drop(&mut self) {
        // Note: This is a synchronous drop, but we need to handle async cleanup
        // In a real implementation, we might use a different approach
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_websocket_client_creation() {
        let config = WebSocketConfig::default();
        let client = WebSocketClient::new(config);
        assert_eq!(client.get_connection_state(), ConnectionState::Disconnected);
    }
    
    #[test]
    fn test_websocket_client_with_url() {
        let client = WebSocketClient::with_default_url("ws://localhost:8000/ws".to_string());
        assert_eq!(client.get_connection_state(), ConnectionState::Disconnected);
    }
    
    #[test]
    fn test_subscription_channel_to_string() {
        assert_eq!(SubscriptionChannel::Transcription.to_string(), "transcription");
        assert_eq!(SubscriptionChannel::Files.to_string(), "files");
        assert_eq!(SubscriptionChannel::Models.to_string(), "models");
        assert_eq!(SubscriptionChannel::System.to_string(), "system");
        
        let task_id = Uuid::new_v4();
        assert_eq!(
            SubscriptionChannel::TranscriptionTask(task_id).to_string(),
            format!("transcription:{}", task_id)
        );
    }
    
    #[test]
    fn test_subscription_channel_from_str() {
        assert_eq!(
            SubscriptionChannel::from_str("transcription"),
            Some(SubscriptionChannel::Transcription)
        );
        assert_eq!(
            SubscriptionChannel::from_str("files"),
            Some(SubscriptionChannel::Files)
        );
        assert_eq!(
            SubscriptionChannel::from_str("models"),
            Some(SubscriptionChannel::Models)
        );
        assert_eq!(
            SubscriptionChannel::from_str("system"),
            Some(SubscriptionChannel::System)
        );
        
        let task_id = Uuid::new_v4();
        assert_eq!(
            SubscriptionChannel::from_str(&format!("transcription:{}", task_id)),
            Some(SubscriptionChannel::TranscriptionTask(task_id))
        );
    }
}