import { forwardRef, type ButtonHTMLAttributes } from 'react';
import { cn } from '../../utils/cn';

type PalButtonTone = "blue" | "violet" | "cyan" | "emerald" | "amber" | "rose" | "zinc";
type PalButtonSize = "sm" | "md" | "lg";
type PalButtonFont = "mono" | "sans";

const BASE_CLASSES =
	"inline-flex items-center justify-center border transition-all duration-200 font-medium focus-visible:outline-none focus-visible:ring-0.5 focus-visible:ring-offset-0 focus-visible:ring-offset-zinc-950 disabled:cursor-not-allowed disabled:opacity-50 rounded-none";

const SIZE_STYLES: Record<PalButtonSize, string> = {
	sm: "px-3 py-1.5 text-xs",
	md: "px-4 py-2 text-sm",
	lg: "px-6 py-3 text-base",
};

const FONT_STYLES: Record<PalButtonFont, string> = {
	mono: "font-mono tracking-tight",
	sans: "font-sans",
};

const TONE_STYLES: Record<PalButtonTone, string> = {
	blue: "focus-visible:ring-blue-500/40 border-blue-500/60 text-blue-100 hover:bg-blue-500/10",
	violet: "focus-visible:ring-violet-500/40 border-violet-500/60 text-violet-100 hover:bg-violet-500/10",
	cyan: "focus-visible:ring-cyan-500/40 border-cyan-500/60 text-cyan-100 hover:bg-cyan-500/10",
	emerald: "focus-visible:ring-emerald-500/40 border-emerald-500/60 text-emerald-100 hover:bg-emerald-500/10",
	amber: "focus-visible:ring-amber-500/40 border-amber-500/60 text-amber-100 hover:bg-amber-500/10",
	rose: "focus-visible:ring-rose-500/40 border-rose-500/60 text-rose-100 hover:bg-rose-500/10",
	zinc: "focus-visible:ring-zinc-500/30 border-zinc-500/60 text-zinc-100 hover:bg-zinc-500/10",
};

const VARIANT_STYLES = {
	default: "bg-zinc-950/60 border-zinc-800/60 text-zinc-100 hover:border-zinc-600/60 hover:bg-zinc-950/80",
	primary: "bg-zinc-950/80 border-zinc-700/60 text-zinc-100 hover:border-zinc-600/80 hover:bg-zinc-950/90",
	secondary: "bg-zinc-900/40 border-zinc-800/50 text-zinc-100 hover:border-zinc-600/60 hover:bg-zinc-900/60",
	ghost: "bg-transparent border-transparent text-zinc-100 hover:bg-zinc-800/30",
	accent: "bg-zinc-950/60 border-zinc-800/60 text-zinc-100 hover:border-zinc-600/60 hover:bg-zinc-950/80",
	outline: "bg-transparent border-zinc-700/60 text-zinc-100 hover:bg-zinc-800/40",
} as const satisfies Record<string, string>;

export interface PalButtonProps
	extends ButtonHTMLAttributes<HTMLButtonElement> {
	variant?: "default" | "primary" | "secondary" | "ghost" | "accent" | "outline";
	tone?: PalButtonTone;
	size?: PalButtonSize;
	font?: PalButtonFont;
	withGlow?: boolean;
	withCornerMarkers?: boolean;
	fullWidth?: boolean;
}

const PalButton = forwardRef<HTMLButtonElement, PalButtonProps>(
	({
		className,
		variant = "primary",
		tone = "cyan",
		size = "md",
		font = "sans",
		withGlow = false,
		withCornerMarkers = false,
		fullWidth = false,
		children,
		disabled,
		...props
	}, ref) => {
		const glowClasses = withGlow ? 'hover:shadow-[0_0_20px_rgba(var(--glow-color),0.3)]' : '';
		const widthClasses = fullWidth ? 'w-full' : '';
		const cornerMarkerClasses = withCornerMarkers ? 'relative' : '';
		
		const classes = cn(
			BASE_CLASSES,
			SIZE_STYLES[size],
			FONT_STYLES[font],
			TONE_STYLES[tone],
			VARIANT_STYLES[variant],
			glowClasses,
			widthClasses,
			cornerMarkerClasses,
			disabled && "opacity-50 cursor-not-allowed",
			className
		);
		
		return (
			<button
				className={classes}
				ref={ref}
				disabled={disabled}
				style={withGlow ? { "--glow-color": "59, 130, 246" } as React.CSSProperties : undefined}
				{...props}
			>
				{withCornerMarkers && (
					<>
						<div className="absolute top-0 left-0 w-2 h-2 border-t border-l border-zinc-600" />
						<div className="absolute top-0 right-0 w-2 h-2 border-t border-r border-zinc-600" />
						<div className="absolute bottom-0 left-0 w-2 h-2 border-b border-l border-zinc-600" />
						<div className="absolute bottom-0 right-0 w-2 h-2 border-b border-r border-zinc-600" />
					</>
				)}
				<span className="relative z-10">{children}</span>
			</button>
		);
	}
);

PalButton.displayName = 'PalButton';

export default PalButton;
