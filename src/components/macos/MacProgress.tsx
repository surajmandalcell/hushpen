import React from "react";


export interface MacProgressProps {
  value: number; // 0-100
  max?: number;
  size?: "small" | "medium" | "large";
  variant?: "determinate" | "indeterminate";
  color?: "blue" | "green" | "orange" | "red" | "gray";
  showLabel?: boolean;
  label?: string;
  className?: string;
}

const MacProgress: React.FC<MacProgressProps> = ({
  value,
  max = 100,
  size = "medium",
  variant = "determinate",
  color = "blue",
  showLabel = false,
  label,
  className = "",
}) => {
  const percentage = Math.min(Math.max((value / max) * 100, 0), 100);

  const containerClasses = ["mac-progress-container", className]
    .filter(Boolean)
    .join(" ");

  const progressClasses = [
    "mac-progress",
    `mac-progress--${size}`,
    `mac-progress--${variant}`,
    `mac-progress--${color}`,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={containerClasses}>
      {(showLabel || label) && (
        <div className="mac-progress-label">
          {label || `${Math.round(percentage)}%`}
        </div>
      )}
      <div className={progressClasses}>
        <div className="mac-progress__track">
          <div
            className="mac-progress__fill"
            style={{
              width: variant === "determinate" ? `${percentage}%` : undefined,
            }}
          >
            {variant === "indeterminate" && (
              <div className="mac-progress__indeterminate-bar" />
            )}
          </div>
        </div>
      </div>
    </div>
  );
};

export default MacProgress;
