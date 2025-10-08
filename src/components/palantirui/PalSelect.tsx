import { forwardRef, type SelectHTMLAttributes } from 'react';
import { cn } from '../../utils/cn';

type PalSelectTone = "blue" | "violet" | "cyan" | "emerald" | "amber" | "rose" | "zinc";
type PalSelectSize = "sm" | "md" | "lg";
type PalSelectFont = "mono" | "sans";

const BASE_CLASSES =
	"w-full border transition-colors duration-200 focus-visible:outline-none focus-visible:ring-0.5 focus-visible:ring-offset-0 focus-visible:ring-offset-zinc-950 disabled:cursor-not-allowed disabled:opacity-50 rounded-none appearance-none cursor-pointer";

const SIZE_STYLES: Record<PalSelectSize, string> = {
	sm: "pl-3 pr-9 py-2 text-xs",
	md: "pl-4 pr-11 py-2.5 text-sm",
	lg: "pl-4 pr-12 py-3 text-base",
};

const SIZE_ARROW_STYLES: Record<PalSelectSize, { backgroundSize: string; backgroundPosition: string }> = {
	sm: { backgroundSize: "14px 14px", backgroundPosition: "right 0.75rem center" },
	md: { backgroundSize: "16px 16px", backgroundPosition: "right 1rem center" },
	lg: { backgroundSize: "18px 18px", backgroundPosition: "right 1rem center" },
};

const FONT_STYLES: Record<PalSelectFont, string> = {
	mono: "font-mono tracking-tight",
	sans: "font-sans",
};

const TONE_STYLES: Record<PalSelectTone, string> = {
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
} as const satisfies Record<string, string>;

// SVG arrow icon as data URL (chevron down)
const ARROW_ICON = `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='24' height='24' viewBox='0 0 24 24' fill='none' stroke='rgb(161, 161, 170)' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpolyline points='6 9 12 15 18 9'%3E%3C/polyline%3E%3C/svg%3E")`;

export interface PalSelectProps
	extends Omit<SelectHTMLAttributes<HTMLSelectElement>, "size"> {
	variant?: "default" | "muted" | "ghost";
	tone?: PalSelectTone;
	size?: PalSelectSize;
	font?: PalSelectFont;
	label?: string;
	error?: string;
	isInvalid?: boolean;
	id?: string;
}

type VariantKey = keyof typeof VARIANT_STYLES;

const createSelectVariant = (variant: VariantKey) => {
	const Component = forwardRef<HTMLSelectElement, PalSelectProps>(
		({
			className,
			tone = "cyan",
			size = "md",
			font = "mono",
			label,
			error,
			isInvalid,
			id,
			children,
			...props
		}, ref) => {
			const toneKey: PalSelectTone = isInvalid || error ? "rose" : tone;
			const arrowStyles = SIZE_ARROW_STYLES[size];
			const selectId = id || `select-${Math.random().toString(36).substring(2, 9)}`;

			return (
				<div className="flex flex-col">
					{label && (
						<label
							htmlFor={selectId}
							className="mb-1 text-sm font-medium text-zinc-300"
						>
							{label}
						</label>
					)}
					<select
						id={selectId}
						ref={ref}
						className={cn(
							BASE_CLASSES,
							SIZE_STYLES[size],
							FONT_STYLES[font],
							TONE_STYLES[toneKey],
							VARIANT_STYLES[variant],
							isInvalid || error ? "border-rose-500/60" : undefined,
							className,
						)}
						style={{
							backgroundImage: ARROW_ICON,
							backgroundRepeat: "no-repeat",
							backgroundSize: arrowStyles.backgroundSize,
							backgroundPosition: arrowStyles.backgroundPosition,
						}}
						{...props}
					>
						{children}
					</select>
					{error && (
						<span className="mt-1 text-sm text-rose-400">{error}</span>
					)}
				</div>
			);
		},
	);

	Component.displayName = `PalSelect.${variant}`;

	return Component;
};

const PalSelect = createSelectVariant("default");
const PalSelectMuted = createSelectVariant("muted");
const PalSelectGhost = createSelectVariant("ghost");

export { PalSelectMuted, PalSelectGhost };
export default PalSelect;
