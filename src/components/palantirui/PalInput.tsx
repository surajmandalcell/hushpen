import { forwardRef, type InputHTMLAttributes } from 'react';
import { cn } from '../../utils/cn';

type PalInputTone = "blue" | "violet" | "cyan" | "emerald" | "amber" | "rose" | "zinc";
type PalInputSize = "sm" | "md" | "lg";
type PalInputFont = "mono" | "sans";

const BASE_CLASSES =
	"w-full border transition-colors duration-200 placeholder:text-zinc-500 focus-visible:outline-none focus-visible:ring-0.5 focus-visible:ring-offset-0 focus-visible:ring-offset-zinc-950 disabled:cursor-not-allowed disabled:opacity-50 rounded-none";

const SIZE_STYLES: Record<PalInputSize, string> = {
	sm: "px-3 py-2 text-xs",
	md: "px-4 py-2.5 text-sm",
	lg: "px-4 py-3 text-base",
};

const FONT_STYLES: Record<PalInputFont, string> = {
	mono: "font-mono tracking-tight",
	sans: "font-sans",
};

const TONE_STYLES: Record<PalInputTone, string> = {
	blue: "focus-visible:ring-blue-500/40 focus-visible:border-blue-400/50",
	violet: "focus-visible:ring-violet-500/40 focus-visible:border-violet-400/50",
	cyan: "focus-visible:ring-cyan-500/40 focus-visible:border-cyan-400/50",
	emerald: "focus-visible:ring-emerald-500/40 focus-visible:border-emerald-400/50",
	amber: "focus-visible:ring-amber-500/40 focus-visible:border-amber-400/60",
	rose: "focus-visible:ring-rose-500/40 focus-visible:border-rose-400/60",
	zinc: "focus-visible:ring-zinc-500/30 focus-visible:border-zinc-400/40",
};

const VARIANT_STYLES = {
	default: "bg-zinc-950/60 border-zinc-800/60 text-zinc-100 hover:border-zinc-600/60",
	muted: "bg-zinc-900/40 border-zinc-800/50 text-zinc-100 hover:border-zinc-600/60",
	ghost: "bg-transparent border-zinc-800/40 text-zinc-100 hover:border-zinc-600/50",
	inline:
		"bg-transparent border-transparent px-0 py-0 text-sm focus-visible:ring-0 focus-visible:border-zinc-600 focus-visible:border-b focus-visible:border-solid",
} as const satisfies Record<string, string>;

export interface PalInputProps
	extends Omit<InputHTMLAttributes<HTMLInputElement>, "size"> {
	variant?: "default" | "muted" | "ghost" | "inline";
	tone?: PalInputTone;
	size?: PalInputSize;
	font?: PalInputFont;
	error?: string;
	isInvalid?: boolean;
	withGlow?: boolean;
	withCornerMarkers?: boolean;
	label?: string;
	id?: string;
}

type VariantKey = keyof typeof VARIANT_STYLES;

const createInputVariant = (variant: VariantKey) => {
	const Component = forwardRef<HTMLInputElement, PalInputProps>(
		({
			className,
			tone = "cyan",
			size = "md",
			font = "mono",
			error,
			isInvalid,
			withGlow = false,
			withCornerMarkers = false,
			label,
			id,
			...props
		}, ref) => {
			const toneKey: PalInputTone = isInvalid || error ? "rose" : tone;
			const inputId = id || `input-${Math.random().toString(36).substring(2, 9)}`;
			
			const glowClasses = withGlow ? 'hover:shadow-[0_0_20px_rgba(var(--glow-color),0.3)]' : '';
			const cornerMarkerClasses = withCornerMarkers ? 'relative' : '';
			
			const classes = cn(
				BASE_CLASSES,
				SIZE_STYLES[size],
				FONT_STYLES[font],
				TONE_STYLES[toneKey],
				VARIANT_STYLES[variant],
				isInvalid || error ? "border-rose-500/60" : undefined,
				glowClasses,
				cornerMarkerClasses,
				className,
			);

			return (
				<div className="flex flex-col">
					{label && (
						<label
							htmlFor={inputId}
							className="mb-1 text-sm font-medium text-zinc-300"
						>
							{label}
						</label>
					)}
					<div className="relative">
						<input
							id={inputId}
							ref={ref}
							className={classes}
							style={withGlow ? { "--glow-color": "59, 130, 246" } as React.CSSProperties : undefined}
							{...props}
						/>
						{withCornerMarkers && (
							<>
								<div className="absolute top-0 left-0 w-2 h-2 border-t border-l border-zinc-600" />
								<div className="absolute top-0 right-0 w-2 h-2 border-t border-r border-zinc-600" />
								<div className="absolute bottom-0 left-0 w-2 h-2 border-b border-l border-zinc-600" />
								<div className="absolute bottom-0 right-0 w-2 h-2 border-b border-r border-zinc-600" />
							</>
						)}
					</div>
					{error && (
						<span className="mt-1 text-sm text-rose-400">{error}</span>
					)}
				</div>
			);
		},
	);

	Component.displayName = `PalInput.${variant}`;

	return Component;
};

const PalInput = createInputVariant("default");
const PalInputMuted = createInputVariant("muted");
const PalInputGhost = createInputVariant("ghost");
const PalInputInline = createInputVariant("inline");

export { PalInputMuted, PalInputGhost, PalInputInline };
export default PalInput;
