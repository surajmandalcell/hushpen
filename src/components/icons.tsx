import React from "react";

type AppLogoMarkProps = React.SVGProps<SVGSVGElement> & {
  title?: string;
};

export const AppLogoMark = ({ title, ...props }: AppLogoMarkProps) => (
  <svg viewBox="0 0 1024 1024" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden={title ? undefined : true} {...props}>
    {title ? <title>{title}</title> : null}
    <path
      d="M 188 512 C 188 356 356 342 512 512 C 668 682 836 668 836 512 C 836 356 668 342 512 512 C 356 682 188 668 188 512"
      stroke="currentColor"
      strokeWidth="84"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);
