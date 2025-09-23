// Accessibility service for ASR Pro
export interface AccessibilitySettings {
    highContrast: boolean;
    reducedMotion: boolean;
    fontSize: 'small' | 'medium' | 'large' | 'extra-large';
    announcements: boolean;
    focusOutline: boolean;
}

export interface AccessibilityAnnouncement {
    message: string;
    priority: 'low' | 'medium' | 'high' | 'urgent';
    category?: string;
}

class AccessibilityService {
    private settings: AccessibilitySettings;
    private announcementListeners: ((announcement: AccessibilityAnnouncement) => void)[] = [];

    constructor() {
        this.settings = this.loadSettings();
        this.setupAccessibilityFeatures();
    }

    private loadSettings(): AccessibilitySettings {
        try {
            const stored = localStorage.getItem('asrpro-accessibility');
            if (stored) {
                return { ...this.getDefaultSettings(), ...JSON.parse(stored) };
            }
        } catch (error) {
            console.error('Failed to load accessibility settings:', error);
        }
        return this.getDefaultSettings();
    }

    private getDefaultSettings(): AccessibilitySettings {
        return {
            highContrast: false,
            reducedMotion: false,
            fontSize: 'medium',
            announcements: true,
            focusOutline: true,
        };
    }

    private setupAccessibilityFeatures(): void {
        // Apply settings to the document
        this.applySettings();

        // Listen for system preference changes
        if (typeof window !== 'undefined') {
            // Listen for reduced motion preference
            const mediaQuery = window.matchMedia('(prefers-reduced-motion: reduce)');
            mediaQuery.addEventListener('change', () => {
                this.updateReducedMotionSetting();
            });

            // Listen for high contrast preference
            const highContrastQuery = window.matchMedia('(prefers-contrast: high)');
            highContrastQuery.addEventListener('change', () => {
                this.updateHighContrastSetting();
            });
        }
    }

    private updateReducedMotionSetting(): void {
        if (typeof window !== 'undefined') {
            const prefersReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
            this.updateSettings({ reducedMotion: prefersReducedMotion });
        }
    }

    private updateHighContrastSetting(): void {
        if (typeof window !== 'undefined') {
            const prefersHighContrast = window.matchMedia('(prefers-contrast: high)').matches;
            this.updateSettings({ highContrast: prefersHighContrast });
        }
    }

    private applySettings(): void {
        const root = document.documentElement;

        // Apply high contrast mode
        if (this.settings.highContrast) {
            root.classList.add('high-contrast');
        } else {
            root.classList.remove('high-contrast');
        }

        // Apply reduced motion
        if (this.settings.reducedMotion) {
            root.classList.add('reduced-motion');
            root.style.setProperty('--animation-duration', '0s');
            root.style.setProperty('--transition-duration', '0s');
        } else {
            root.classList.remove('reduced-motion');
            root.style.setProperty('--animation-duration', '0.3s');
            root.style.setProperty('--transition-duration', '0.2s');
        }

        // Apply font size
        const fontSizeMap = {
            small: '12px',
            medium: '14px',
            large: '16px',
            'extra-large': '18px',
        };
        root.style.setProperty('--base-font-size', fontSizeMap[this.settings.fontSize]);

        // Apply focus outline
        if (!this.settings.focusOutline) {
            root.classList.add('no-focus-outline');
        } else {
            root.classList.remove('no-focus-outline');
        }
    }

    getSettings(): AccessibilitySettings {
        return { ...this.settings };
    }

    updateSettings(newSettings: Partial<AccessibilitySettings>): void {
        this.settings = { ...this.settings, ...newSettings };
        this.saveSettings();
        this.applySettings();
    }

    private saveSettings(): void {
        try {
            localStorage.setItem('asrpro-accessibility', JSON.stringify(this.settings));
        } catch (error) {
            console.error('Failed to save accessibility settings:', error);
        }
    }

    // Announcement system for screen readers
    announce(announcement: AccessibilityAnnouncement): void {
        if (!this.settings.announcements) return;

        // Create a live region announcement
        const announcementElement = document.createElement('div');
        announcementElement.setAttribute('aria-live', announcement.priority === 'urgent' ? 'assertive' : 'polite');
        announcementElement.setAttribute('aria-atomic', 'true');
        announcementElement.style.position = 'absolute';
        announcementElement.style.left = '-10000px';
        announcementElement.style.width = '1px';
        announcementElement.style.height = '1px';
        announcementElement.style.overflow = 'hidden';

        announcementElement.textContent = announcement.message;
        document.body.appendChild(announcementElement);

        // Notify listeners
        this.announcementListeners.forEach(listener => {
            try {
                listener(announcement);
            } catch (error) {
                console.error('Error notifying accessibility listener:', error);
            }
        });

        // Remove the element after announcement
        setTimeout(() => {
            if (document.body.contains(announcementElement)) {
                document.body.removeChild(announcementElement);
            }
        }, 1000);
    }

    subscribeToAnnouncements(listener: (announcement: AccessibilityAnnouncement) => void): () => void {
        this.announcementListeners.push(listener);
        return () => {
            const index = this.announcementListeners.indexOf(listener);
            if (index > -1) {
                this.announcementListeners.splice(index, 1);
            }
        };
    }

    // Focus management utilities
    trapFocus(container: HTMLElement): () => void {
        const focusableElements = container.querySelectorAll(
            'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
        );
        const firstElement = focusableElements[0] as HTMLElement;
        const lastElement = focusableElements[focusableElements.length - 1] as HTMLElement;

        const handleTabKey = (e: KeyboardEvent) => {
            if (e.key !== 'Tab') return;

            if (e.shiftKey) {
                if (document.activeElement === firstElement) {
                    e.preventDefault();
                    lastElement?.focus();
                }
            } else {
                if (document.activeElement === lastElement) {
                    e.preventDefault();
                    firstElement?.focus();
                }
            }
        };

        container.addEventListener('keydown', handleTabKey);

        // Return cleanup function
        return () => {
            container.removeEventListener('keydown', handleTabKey);
        };
    }

    // Accessibility testing utilities
    checkContrastRatio(foreground: string, background: string): number {
        // Simple contrast ratio calculation (would use a more robust library in production)
        const getLuminance = (hex: string): number => {
            const r = parseInt(hex.substr(1, 2), 16) / 255;
            const g = parseInt(hex.substr(3, 2), 16) / 255;
            const b = parseInt(hex.substr(5, 2), 16) / 255;

            const [rs, gs, bs] = [r, g, b].map(c => {
                return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
            });

            return 0.2126 * rs + 0.7152 * gs + 0.0722 * bs;
        };

        const l1 = getLuminance(foreground);
        const l2 = getLuminance(background);

        const ratio = (Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05);
        return Math.round(ratio * 100) / 100;
    }

    // Keyboard navigation utilities
    setupKeyboardNavigation(container: HTMLElement): () => void {
        const handleKeyNavigation = (e: KeyboardEvent) => {
            switch (e.key) {
                case 'ArrowUp':
                case 'ArrowDown':
                case 'ArrowLeft':
                case 'ArrowRight':
                case 'Home':
                case 'End':
                case 'PageUp':
                case 'PageDown':
                case 'Enter':
                case ' ':
                    // Let the component handle these keys
                    break;
                case 'Escape':
                    // Close modals, overlays, etc.
                    this.announce({
                        message: 'Closed',
                        priority: 'medium',
                    });
                    break;
            }
        };

        container.addEventListener('keydown', handleKeyNavigation);

        return () => {
            container.removeEventListener('keydown', handleKeyNavigation);
        };
    }
}

// Export singleton instance
export const accessibilityService = new AccessibilityService();

// Export React hook for using accessibility settings
import React from 'react';

export const useAccessibility = () => {
    const [settings, setSettings] = React.useState(accessibilityService.getSettings());

    React.useEffect(() => {
        const handleSettingsChange = () => {
            setSettings(accessibilityService.getSettings());
        };

        // For now, we'll just get the initial settings
        // In a more complete implementation, we'd listen for changes
        const interval = setInterval(handleSettingsChange, 1000);

        return () => {
            clearInterval(interval);
        };
    }, []);

    return {
        settings,
        updateSettings: accessibilityService.updateSettings.bind(accessibilityService),
        announce: accessibilityService.announce.bind(accessibilityService),
    };
};
