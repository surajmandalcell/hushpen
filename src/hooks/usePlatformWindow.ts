import { useEffect } from 'react';
import { platformService } from '../services/platform';

export const usePlatformWindow = () => {
    useEffect(() => {
        // Apply platform-specific behaviors
        if (platformService.isDesktop()) {
            // Desktop-specific behaviors
            document.body.classList.add('platform-desktop');

            // Set platform-specific CSS variables
            const root = document.documentElement;
            root.style.setProperty('--platform-titlebar-height', `${platformService.getWindowTitleBarHeight()}px`);

            // Platform-specific window behaviors
            if (platformService.isMacOS()) {
                // macOS-specific behaviors
                document.body.classList.add('platform-macos');

                // Handle window controls differently on macOS
                const trafficLights = document.querySelector('.mac-traffic-lights');
                if (trafficLights) {
                    trafficLights.classList.add('macos-native');
                }

                // macOS uses different keyboard shortcuts
                document.addEventListener('keydown', (e) => {
                    // Cmd+, for preferences (macOS convention)
                    if (e.metaKey && e.key === ',') {
                        e.preventDefault();
                        // Could dispatch an event to open settings
                        console.log('Preferences shortcut pressed (macOS)');
                    }
                });
            } else if (platformService.isWindows()) {
                // Windows-specific behaviors
                document.body.classList.add('platform-windows');

                // Windows uses Ctrl+, for preferences
                document.addEventListener('keydown', (e) => {
                    if (e.ctrlKey && e.key === ',') {
                        e.preventDefault();
                        // Could dispatch an event to open settings
                        console.log('Preferences shortcut pressed (Windows)');
                    }
                });
            } else if (platformService.isLinux()) {
                // Linux-specific behaviors
                document.body.classList.add('platform-linux');
            }

            // Platform-specific drag behavior for frameless windows
            const setupDragZones = () => {
                const dragZones = document.querySelectorAll('.drag-zone');
                dragZones.forEach(zone => {
                    if (platformService.isMacOS()) {
                        zone.setAttribute('data-platform', 'macos');
                    } else if (platformService.isWindows()) {
                        zone.setAttribute('data-platform', 'windows');
                    } else {
                        zone.setAttribute('data-platform', 'linux');
                    }
                });
            };

            setupDragZones();

            // Platform-specific focus management
            const handleFocusChange = () => {
                if (platformService.isMacOS()) {
                    document.body.classList.add('macos-focused');
                } else {
                    document.body.classList.remove('macos-focused');
                }
            };

            window.addEventListener('focus', handleFocusChange);
            window.addEventListener('blur', handleFocusChange);

            return () => {
                window.removeEventListener('focus', handleFocusChange);
                window.removeEventListener('blur', handleFocusChange);
                document.body.classList.remove('platform-desktop', 'platform-macos', 'platform-windows', 'platform-linux');
            };
        }
    }, []);

    return {
        platform: platformService.getPlatform(),
        isMacOS: platformService.isMacOS(),
        isWindows: platformService.isWindows(),
        isLinux: platformService.isLinux(),
        shouldUseNativeWindowControls: platformService.shouldUseNativeWindowControls(),
        commandKey: platformService.getCommandKey(),
        altKey: platformService.getAltKey(),
    };
};
