import type { ReactNode } from "react";

interface ViewFrameProps {
  title: string;
  children: ReactNode;
}

export function ViewFrame({ title, children }: ViewFrameProps) {
  return (
    <section className="mx-auto w-full max-w-[520px] space-y-4">
      <h2 className="sr-only">{title}</h2>
      {children}
    </section>
  );
}
