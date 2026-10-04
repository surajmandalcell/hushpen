import type { ReactNode } from "react";

interface StatusLabelProps {
  children: ReactNode;
}

export function StatusLabel({ children }: StatusLabelProps) {
  return <span className="text-[12px] font-semibold text-[#cfcfcf]">{children}</span>;
}
