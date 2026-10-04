import { segmentedControlClass, segmentedItemClass } from "./classes";

export interface SegmentedControlOption<TValue extends string> {
  value: TValue;
  label: string;
  ariaLabel: string;
}

interface SegmentedControlProps<TValue extends string> {
  value: TValue;
  options: readonly SegmentedControlOption<TValue>[];
  onChange: (value: TValue) => void;
}

export function SegmentedControl<TValue extends string>({ value, options, onChange }: SegmentedControlProps<TValue>) {
  return (
    <div className={segmentedControlClass}>
      {options.map((option) => {
        const active = value === option.value;

        return (
          <button
            key={option.value}
            type="button"
            aria-label={option.ariaLabel}
            aria-pressed={active}
            className={`${segmentedItemClass} ${active ? "bg-[#686868] text-white" : "text-[#aaa] hover:text-white"}`}
            onClick={() => onChange(option.value)}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
