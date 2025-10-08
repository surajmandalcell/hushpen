// Platform detection and platform-specific behaviors for ASR Pro
export type Platform = 'macos' | 'windows' | 'linux' | 'unknown';

export interface PlatformInfo {
    platform: Platform;
    isMacOS: () => boolean;
    isWindows: () => boolean;
    isLinux: () => boolean;
    isWeb: boolean;
    isDesktop: boolean;
    arch: string;
    version: string;
    getPlatformCSSClass: () => string;
    getWindowTitleBarHeight: () => number;
    getCommandKey: () => string;
    getAltKey: () => string;
    shouldUseNativeWindowControls: () => boolean;
}

class PlatformService {
    private platformInfo: PlatformInfo;

    constructor() {
        this.platformInfo = this.detectPlatform();
    }

    private detectPlatform(): PlatformInfo {
        // Check if running in Electron environment
        const isElectron = typeof window !== 'undefined' &&
            (window as any).electronAPI;
        
        // Check if running in Tauri environment (for backward compatibility)
        const isTauri = typeof window !== 'undefined' &&
            (window as any).__TAURI__ &&
            (window as any).__TAURI_INTERNALS__;

        // Get platform information
        const platform = navigator.platform.toLowerCase();

        let currentPlatform: Platform = 'unknown';
        let isMacOS = false;
        let isWindows = false;
        let isLinux = false;

        if (platform.includes('mac') || platform.includes('darwin')) {
            currentPlatform = 'macos';
            isMacOS = true;
        } else if (platform.includes('win')) {
            currentPlatform = 'windows';
            isWindows = true;
        } else if (platform.includes('linux')) {
            currentPlatform = 'linux';
            isLinux = true;
        }

        const platformInfo: PlatformInfo = {
            platform: currentPlatform,
            isMacOS: () => isMacOS,
            isWindows: () => isWindows,
            isLinux: () => isLinux,
            isWeb: !(isElectron || isTauri),
            isDesktop: isElectron || isTauri,
            arch: navigator.userAgent.includes('x64') || navigator.userAgent.includes('x86_64') ? 'x64' : 'x86',
            version: navigator.platform,
            getPlatformCSSClass: () => `platform-${currentPlatform}`,
            getWindowTitleBarHeight: () => this.getWindowTitleBarHeight(),
            getCommandKey: () => this.getCommandKey(),
            getAltKey: () => this.getAltKey(),
            shouldUseNativeWindowControls: () => this.shouldUseNativeWindowControls(),
        };

        return platformInfo;
    }

    getPlatformInfo(): PlatformInfo {
        return this.platformInfo;
    }

    getPlatform(): Platform {
        return this.platformInfo.platform;
    }

    isMacOS(): boolean {
        return this.platformInfo.isMacOS();
    }

    isWindows(): boolean {
        return this.platformInfo.isWindows();
    }

    isLinux(): boolean {
        return this.platformInfo.isLinux();
    }

    isDesktop(): boolean {
        return this.platformInfo.isDesktop;
    }

    // Platform-specific keyboard shortcuts
    getCommandKey(): string {
        return this.platformInfo.getCommandKey();
    }

    getAltKey(): string {
        return this.platformInfo.getAltKey();
    }

    // Platform-specific file paths
    getDefaultAudioPath(): string {
        if (this.platformInfo.isMacOS()) {
            return '~/Music';
        } else if (this.platformInfo.isWindows()) {
            return '%USERPROFILE%\\Music';
        } else {
            return '~/Music';
        }
    }

    getDefaultDownloadsPath(): string {
        if (this.platformInfo.isMacOS()) {
            return '~/Downloads';
        } else if (this.platformInfo.isWindows()) {
            return '%USERPROFILE%\\Downloads';
        } else {
            return '~/Downloads';
        }
    }

    // Platform-specific behaviors
    shouldUseNativeWindowControls(): boolean {
        return this.platformInfo.shouldUseNativeWindowControls();
    }

    shouldShowSystemTrayIcon(): boolean {
        // On macOS, we typically don't show tray icons for regular apps
        const isDevelopment = typeof window !== 'undefined' &&
            window.location.hostname === 'localhost';
        return !this.platformInfo.isMacOS() || isDevelopment;
    }

    getWindowTitleBarHeight(): number {
        return this.platformInfo.getWindowTitleBarHeight();
    }

    // Format file sizes for different platforms
    formatFileSize(bytes: number): string {
        const units = ['B', 'KB', 'MB', 'GB'];
        let size = bytes;
        let unitIndex = 0;

        while (size >= 1024 && unitIndex < units.length - 1) {
            size /= 1024;
            unitIndex++;
        }

        return `${size.toFixed(1)} ${units[unitIndex]}`;
    }

    // Platform-specific error messages
    getMicrophonePermissionErrorMessage(): string {
        if (this.platformInfo.isMacOS()) {
            return 'Please allow microphone access in System Preferences > Security & Privacy > Microphone';
        } else if (this.platformInfo.isWindows()) {
            return 'Please allow microphone access in Windows Settings > Privacy > Microphone';
        } else {
            return 'Please allow microphone access in your system settings';
        }
    }

    // Platform-specific styling classes
    getPlatformCSSClass(): string {
        return this.platformInfo.getPlatformCSSClass();
    }

    // Platform-specific behaviors for file operations
    getSupportedAudioFormats(): string[] {
        // All platforms support the same formats, but we can extend this later
        return ['mp3', 'wav', 'm4a', 'flac', 'ogg'];
    }

    // Check if platform supports certain features
    supportsGlobalHotkeys(): boolean {
        return this.platformInfo.isDesktop;
    }

    supportsNativeFileDialogs(): boolean {
        return this.platformInfo.isDesktop;
    }

    supportsSystemNotifications(): boolean {
        return this.platformInfo.isDesktop || 'Notification' in window;
    }
}

// Export singleton instance
export const platformService = new PlatformService();

// Export React hook for using platform info
import React from 'react';

export const usePlatform = () => {
    const [platformInfo, setPlatformInfo] = React.useState(platformService.getPlatformInfo());

    React.useEffect(() => {
        // Re-detect platform if needed (though it should be static)
        setPlatformInfo(platformService.getPlatformInfo());
    }, []);

    return platformInfo;
};
