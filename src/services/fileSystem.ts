// File system service for native dialogs and file operations
// Check if we're in Electron environment
const isElectron = typeof window !== 'undefined' && (window as any).electronAPI;

// Conditional imports based on environment
let tauriOpen: any, tauriReadTextFile: any, tauriWriteTextFile: any, tauriReadFile: any;

if (!isElectron) {
    // Import Tauri APIs only if not in Electron
    import('@tauri-apps/plugin-dialog').then(module => {
        tauriOpen = module.open;
    });
    import('@tauri-apps/plugin-fs').then(module => {
        tauriReadTextFile = module.readTextFile;
        tauriWriteTextFile = module.writeTextFile;
        tauriReadFile = module.readFile;
    });
}

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
            if (isElectron) {
                // Use Electron API
                const filePath = await (window as any).electronAPI.openFile();
                return filePath ? [filePath] : null;
            } else {
                // Use Tauri API
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

                const selected = await tauriOpen({
                    multiple: options?.multiple || false,
                    directory: options?.directory || false,
                    defaultPath: options?.defaultPath,
                    filters: options?.filters || defaultFilters,
                });

                if (selected) {
                    return Array.isArray(selected) ? selected : [selected];
                }

                return null;
            }
        } catch (error) {
            console.error('Failed to open file dialog:', error);
            throw error;
        }
    }

    async openDirectoryDialog(defaultPath?: string): Promise<string | null> {
        try {
            if (isElectron) {
                // For Electron, we would need to implement a directory dialog
                // For now, return null or use the regular file dialog
                return null;
            } else {
                // Use Tauri API
                const selected = await tauriOpen({
                    directory: true,
                    defaultPath,
                });

                return selected as string | null;
            }
        } catch (error) {
            console.error('Failed to open directory dialog:', error);
            throw error;
        }
    }

    async readTextFile(path: string): Promise<string> {
        try {
            if (isElectron) {
                // For Electron, we would need to implement file reading
                // For now, use fetch as a fallback
                const response = await fetch(`file://${path}`);
                return await response.text();
            } else {
                // Use Tauri API
                return await tauriReadTextFile(path);
            }
        } catch (error) {
            console.error('Failed to read text file:', error);
            throw error;
        }
    }

    async readBinaryFile(path: string): Promise<Uint8Array> {
        try {
            if (isElectron) {
                // For Electron, we would need to implement binary file reading
                // For now, use fetch as a fallback
                const response = await fetch(`file://${path}`);
                const buffer = await response.arrayBuffer();
                return new Uint8Array(buffer);
            } else {
                // Use Tauri API
                return await tauriReadFile(path);
            }
        } catch (error) {
            console.error('Failed to read binary file:', error);
            throw error;
        }
    }

    async writeTextFile(path: string, content: string): Promise<void> {
        try {
            if (isElectron) {
                // For Electron, we would need to implement file writing
                // For now, just log the content
                console.log('Write to file:', path, content);
            } else {
                // Use Tauri API
                await tauriWriteTextFile(path, content);
            }
        } catch (error) {
            console.error('Failed to write text file:', error);
            throw error;
        }
    }

    async saveFileDialog(content: string, defaultPath?: string, defaultName?: string): Promise<string | null> {
        try {
            if (isElectron) {
                // Use Electron API
                const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
                const filename = defaultName || `transcription-${timestamp}.txt`;
                const success = await (window as any).electronAPI.saveFile(content, filename);
                return success ? filename : null;
            } else {
                // For Tauri, we'll use a simple save dialog approach
                const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
                const filename = defaultName || `transcription-${timestamp}.txt`;
                const path = defaultPath ? `${defaultPath}/${filename}` : filename;

                await this.writeTextFile(path, content);
                return path;
            }
        } catch (error) {
            console.error('Failed to save file:', error);
            throw error;
        }
    }
}

// Export singleton instance
export const fileSystemService = new FileSystemServiceImpl();
