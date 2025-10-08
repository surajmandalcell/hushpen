import type { ReactNode } from 'react';

// Base Components
export { default as PalButton } from './PalButton';
export type { PalButtonProps } from './PalButton';

export { default as PalInput, PalInputMuted, PalInputGhost, PalInputInline } from './PalInput';
export type { PalInputProps } from './PalInput';

export { default as PalSelect, PalSelectMuted, PalSelectGhost } from './PalSelect';
export type { PalSelectProps } from './PalSelect';

export { default as PalCard } from './PalCard';
export type { PalCardProps } from './PalCard';

export { default as PalIcon } from './PalIcon';
export type { PalIconProps } from './PalIcon';

export { default as PalText } from './PalText';
export type { PalTextProps } from './PalText';

// Composite Components
export { default as PalSidebar, PalSidebarItem, PalSidebarSection } from './PalSidebar';
export type { PalSidebarProps, PalSidebarItemProps, PalSidebarSectionProps } from './PalSidebar';

export {
  default as PalHeader,
  PalWindowControls,
  PalWindowControl,
  PalHeaderTitle,
  PalHeaderActions
} from './PalHeader';
export type {
  PalHeaderProps,
  PalWindowControlsProps,
  PalWindowControlProps,
  PalHeaderTitleProps,
  PalHeaderActionsProps
} from './PalHeader';

export {
  default as PalPanel,
  PalPanelHeader,
  PalPanelContent,
  PalPanelFooter
} from './PalPanel';
export type {
  PalPanelProps,
  PalPanelHeaderProps,
  PalPanelContentProps,
  PalPanelFooterProps
} from './PalPanel';

export {
  default as PalModal,
  PalModalHeader,
  PalModalContent,
  PalModalFooter
} from './PalModal';
export type {
  PalModalProps,
  PalModalHeaderProps,
  PalModalContentProps,
  PalModalFooterProps
} from './PalModal';

// Utility Types
export interface PalantirUIComponentProps {
  className?: string;
  children?: ReactNode;
}

export interface PalantirUIVariantProps {
  variant?: 'default' | 'primary' | 'secondary' | 'ghost' | 'muted' | 'accent';
  size?: 'xs' | 'sm' | 'md' | 'lg' | 'xl';
}

export interface PalantirUIInteractiveProps {
  disabled?: boolean;
  withGlow?: boolean;
  withCornerMarkers?: boolean;
  withGeometricBorder?: boolean;
}

// Re-export utility function
export { cn } from '../../utils/cn';
