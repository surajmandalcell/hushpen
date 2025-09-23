// File system service for native dialogs and file operations
import { open } from '@tauri-apps/plugin-dialog';
import { readTextFile, writeTextFile } from '@tauri-apps/plugin-fs';
import { readFile } from '@tauri-apps/plugin-fs';

export interface FileSystemService {
    openFileDialog: (options?: {
        multiple?: boolean;
        directory?: boolean;
        defaultPath?: string;
        filters?: { name: string; extensions: string[] }[];
    }) => Promise<string[] | null>;

    openDirectoryDialog: (defaultPath?: string) => Promise<string | null>;

    readTextFile: (path: string) => Promise<string>;

    readBinaryFile: (path: string) => Promise<Uint8Array>;

    writeTextFile: (path: string, content: string) => Promise<void>;

    saveFileDialog: (content: string, defaultPath?: string, defaultName?: string) => Promise<string | null>;
}

class FileSystemServiceImpl implements FileSystemService {
    async openFileDialog(options?: {
        multiple?: boolean;
        directory?: boolean;
        defaultPath?: string;
        filters?: { name: string; extensions: string[] }[];
    }): Promise<string[] | null> {
        try {
            const defaultFilters = [
                {
                    name: 'Audio Files',
                    extensions: ['mp3', 'wav', 'm4a', 'flac', 'ogg'],
                },
                {
                    name: 'All Files',
                    extensions: ['*'],
                },
            ];

            const selected = await open({
                multiple: options?.multiple || false,
                directory: options?.directory || false,
                defaultPath: options?.defaultPath,
                filters: options?.filters || defaultFilters,
            });

            if (selected) {
                return Array.isArray(selected) ? selected : [selected];
            }

            return null;
        } catch (error) {
            console.error('Failed to open file dialog:', error);
            throw error;
        }
    }

    async openDirectoryDialog(defaultPath?: string): Promise<string | null> {
        try {
            const selected = await open({
                directory: true,
                defaultPath,
            });

            return selected as string | null;
        } catch (error) {
            console.error('Failed to open directory dialog:', error);
            throw error;
        }
    }

    async readTextFile(path: string): Promise<string> {
        try {
            return await readTextFile(path);
        } catch (error) {
            console.error('Failed to read text file:', error);
            throw error;
        }
    }

    async readBinaryFile(path: string): Promise<Uint8Array> {
        try {
            return await readFile(path);
        } catch (error) {
            console.error('Failed to read binary file:', error);
            throw error;
        }
    }

    async writeTextFile(path: string, content: string): Promise<void> {
        try {
            await writeTextFile(path, content);
        } catch (error) {
            console.error('Failed to write text file:', error);
            throw error;
        }
    }

    async saveFileDialog(content: string, defaultPath?: string, defaultName?: string): Promise<string | null> {
        try {
            // For now, we'll use a simple save dialog approach
            // In a more complete implementation, we'd show a save dialog first
            const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
            const filename = defaultName || `transcription-${timestamp}.txt`;
            const path = defaultPath ? `${defaultPath}/${filename}` : filename;

            await this.writeTextFile(path, content);
            return path;
        } catch (error) {
            console.error('Failed to save file:', error);
            throw error;
        }
    }
}

// Export singleton instance
export const fileSystemService = new FileSystemServiceImpl();
