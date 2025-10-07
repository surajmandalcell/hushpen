import { useEffect, useState } from 'react';

// WebSocket service for real-time communication with Python sidecar
export interface WebSocketMessage {
    type: 'transcription_progress' | 'transcription_started' | 'transcription_completed' | 'transcription_error' |
          'model_status' | 'container_status' | 'system_status' | 'error' | 'info' | 'ping' | 'pong';
    data: any;
}

export interface ContainerStatusData {
    model_id?: string;
    status?: string;
    gpu_allocated?: boolean;
    image?: string;
}

export interface SystemStatusData {
    docker_available?: boolean;
    gpu_available?: boolean;
    current_model?: string;
    models?: string[];
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

export const useWebSocket = () => {
    const [isConnected, setIsConnected] = useState(false);
    const [containerStatuses, setContainerStatuses] = useState<Record<string, ContainerStatusData>>({});
    const [systemStatus, setSystemStatus] = useState<SystemStatusData>({});

    useEffect(() => {
        const checkConnection = () => {
            setIsConnected(webSocketService.isConnected());
        };

        const unsubscribe = webSocketService.subscribe((message) => {
            // Handle WebSocket messages
            console.log('WebSocket message:', message);
            
            switch (message.type) {
                case 'container_status': {
                    const containerData = message.data as ContainerStatusData;
                    const { model_id } = containerData;
                    if (!model_id) {
                        return;
                    }

                    setContainerStatuses(prev => ({
                        ...prev,
                        [model_id]: containerData
                    }));
                    break;
                }
                    
                case 'system_status':
                    setSystemStatus(message.data as SystemStatusData);
                    break;
                    
                case 'transcription_started':
                    console.log('Transcription started:', message.data);
                    break;
                    
                case 'transcription_progress':
                    console.log('Transcription progress:', message.data);
                    break;
                    
                case 'transcription_completed':
                    console.log('Transcription completed:', message.data);
                    break;
                    
                case 'transcription_error':
                    console.error('Transcription error:', message.data);
                    break;
                    
                default:
                    // Handle other message types
                    break;
            }
        });

        const interval = setInterval(checkConnection, 1000);

        return () => {
            clearInterval(interval);
            unsubscribe();
        };
    }, []);

    return {
        isConnected,
        containerStatuses,
        systemStatus,
        send: webSocketService.send.bind(webSocketService),
    };
};
