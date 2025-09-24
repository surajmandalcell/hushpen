import React from "react";


export interface MacScrollbarProps {
  children: React.ReactNode;
  className?: string;
  horizontal?: boolean;
  vertical?: boolean;
  autoHide?: boolean;
  thin?: boolean;
}

const MacScrollbar: React.FC<MacScrollbarProps> = ({
  children,
  className = "",
  horizontal = false,
  vertical = true,
  autoHide = true,
  thin = true,
}) => {
  const containerClasses = [
    "mac-scrollbar-container",
    horizontal ? "mac-scrollbar-container--horizontal" : "",
    vertical ? "mac-scrollbar-container--vertical" : "",
    autoHide ? "mac-scrollbar-container--auto-hide" : "",
    thin ? "mac-scrollbar-container--thin" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={containerClasses}>
      <div className="mac-scrollbar-content">{children}</div>
    </div>
  );
};

export default MacScrollbar;
