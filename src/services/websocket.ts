// WebSocket service for real-time communication with Python sidecar
export interface WebSocketMessage {
    type: 'transcription_progress' | 'transcription_started' | 'transcription_completed' | 'model_status' | 'error' | 'info';
    data: any;
}

class WebSocketService {
    private ws: WebSocket | null = null;
    private listeners: ((message: WebSocketMessage) => void)[] = [];
    private reconnectAttempts = 0;
    private maxReconnectAttempts = 5;
    private reconnectDelay = 1000; // Start with 1 second

    constructor() {
        this.connect();
    }

    private connect(): void {
        try {
            this.ws = new WebSocket('ws://localhost:8000/ws');

            this.ws.onopen = () => {
                console.log('WebSocket connected to Python sidecar');
                this.reconnectAttempts = 0;
                this.reconnectDelay = 1000;
            };

            this.ws.onmessage = (event) => {
                try {
                    const message: WebSocketMessage = JSON.parse(event.data);
                    this.notifyListeners(message);
                } catch (error) {
                    console.error('Failed to parse WebSocket message:', error);
                }
            };

            this.ws.onerror = (error) => {
                console.error('WebSocket error:', error);
            };

            this.ws.onclose = (event) => {
                console.log('WebSocket connection closed:', event.code, event.reason);
                this.attemptReconnect();
            };

        } catch (error) {
            console.error('Failed to connect to WebSocket:', error);
            this.attemptReconnect();
        }
    }

    private attemptReconnect(): void {
        if (this.reconnectAttempts < this.maxReconnectAttempts) {
            this.reconnectAttempts++;
            console.log(`Attempting to reconnect... (${this.reconnectAttempts}/${this.maxReconnectAttempts})`);

            setTimeout(() => {
                this.connect();
            }, this.reconnectDelay);

            // Exponential backoff
            this.reconnectDelay *= 2;
        } else {
            console.error('Max reconnection attempts reached');
        }
    }

    subscribe(listener: (message: WebSocketMessage) => void): () => void {
        this.listeners.push(listener);
        return () => {
            const index = this.listeners.indexOf(listener);
            if (index > -1) {
                this.listeners.splice(index, 1);
            }
        };
    }

    private notifyListeners(message: WebSocketMessage): void {
        this.listeners.forEach(listener => {
            try {
                listener(message);
            } catch (error) {
                console.error('Error notifying WebSocket listener:', error);
            }
        });
    }

    send(message: any): void {
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify(message));
        } else {
            console.warn('WebSocket not connected, message not sent:', message);
        }
    }

    disconnect(): void {
        if (this.ws) {
            this.ws.close();
            this.ws = null;
        }
    }

    isConnected(): boolean {
        return this.ws?.readyState === WebSocket.OPEN;
    }
}

// Export singleton instance
export const webSocketService = new WebSocketService();

// React hook for WebSocket
import React from 'react';

export const useWebSocket = () => {
    const [isConnected, setIsConnected] = React.useState(false);

    React.useEffect(() => {
        const checkConnection = () => {
            setIsConnected(webSocketService.isConnected());
        };

        const unsubscribe = webSocketService.subscribe((message) => {
            // Handle WebSocket messages
            console.log('WebSocket message:', message);
        });

        const interval = setInterval(checkConnection, 1000);

        return () => {
            clearInterval(interval);
            unsubscribe();
        };
    }, []);

    return {
        isConnected,
        send: webSocketService.send.bind(webSocketService),
    };
};
