import { useEffect } from 'react';
import { webSocketService } from '../services/websocket';

export const useWebSocket = () => {
    useEffect(() => {
        // Initialize WebSocket connection
        const unsubscribe = webSocketService.subscribe((message) => {
            console.log('WebSocket message received:', message);
            // Handle different message types
            switch (message.type) {
                case 'transcription_progress':
                    // Handle transcription progress updates
                    console.log('Transcription progress:', message.data);
                    break;
                case 'transcription_started':
                    console.log('Transcription started:', message.data);
                    break;
                case 'transcription_completed':
                    console.log('Transcription completed:', message.data);
                    break;
                case 'model_status':
                    console.log('Model status:', message.data);
                    break;
                case 'error':
                    console.error('WebSocket error:', message.data);
                    break;
                default:
                    console.log('Unknown WebSocket message:', message);
            }
        });

        return unsubscribe;
    }, []);

    return {
        isConnected: webSocketService.isConnected(),
        send: webSocketService.send.bind(webSocketService),
    };
};
