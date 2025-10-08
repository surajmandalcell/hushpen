// API client for ASR Pro Electron backend
export interface HealthResponse {
    status: string;
    current_model?: string;
    device?: string;
}

export interface Model {
    id: string;
    object: string;
    owned_by: string;
    ready: boolean;
}

export interface ModelListResponse {
    object: string;
    data: Model[];
}

export interface ModelSettingRequest {
    model_id: string;
}

export interface ModelSettingResponse {
    status: string;
    model: string;
}

export interface TranscriptionResponse {
    text: string;
    language?: string;
    language_probability?: number;
    duration?: number;
    segments?: Array<{
        start: number;
        end: number;
        text: string;
    }>;
    // Docker-specific fields
    model_id?: string;
    processing_time?: number;
    backend?: string;
    container_info?: {
        status?: string;
        gpu_allocated?: boolean;
        image?: string;
    };
}

export interface ApiError {
    detail: string;
}

class ApiClient {
    private baseUrl: string;

    constructor() {
        // Default to localhost:3001 for the Electron backend
        this.baseUrl = 'http://localhost:3001';
    }

    private async request<T>(endpoint: string, options?: RequestInit): Promise<T> {
        const url = `${this.baseUrl}${endpoint}`;

        const response = await fetch(url, {
            headers: {
                'Content-Type': 'application/json',
                ...options?.headers,
            },
            ...options,
        });

        if (!response.ok) {
            const errorData: ApiError = await response.json().catch(() => ({ detail: 'Unknown error' }));
            throw new Error(errorData.detail || `HTTP ${response.status}`);
        }

        return response.json();
    }

    async healthCheck(): Promise<HealthResponse> {
        return this.request<HealthResponse>('/health');
    }

    async listModels(): Promise<ModelListResponse> {
        return this.request<ModelListResponse>('/v1/models');
    }

    async setModel(modelId: string): Promise<ModelSettingResponse> {
        return this.request<ModelSettingResponse>('/v1/settings/model', {
            method: 'POST',
            body: JSON.stringify({ model_id: modelId }),
        });
    }

    async transcribeFile(file: File, model?: string, responseFormat: 'json' | 'text' | 'srt' = 'json'): Promise<any> {
        const formData = new FormData();
        formData.append('file', file);

        if (model) {
            formData.append('model', model);
        }

        formData.append('response_format', responseFormat);

        const response = await fetch(`${this.baseUrl}/v1/audio/transcriptions`, {
            method: 'POST',
            body: formData,
        });

        if (!response.ok) {
            const errorData: ApiError = await response.json().catch(() => ({ detail: 'Unknown error' }));
            throw new Error(errorData.detail || `HTTP ${response.status}`);
        }

        // Handle different response formats
        const contentType = response.headers.get('content-type');
        if (contentType?.includes('text/plain') || responseFormat === 'srt') {
            return response.text();
        }

        return response.json();
    }
}

// Export singleton instance
export const apiClient = new ApiClient();
