// SPDX-License-Identifier: Apache-2.0

import type { ReactNode } from "react";

type IconProps = { className?: string };

const Svg = ({ className, children }: IconProps & { children: ReactNode }) => (
  <svg
    className={className}
    width="16"
    height="16"
    viewBox="0 0 24 24"
    fill="none"
    aria-hidden="true"
    focusable="false"
  >
    {children}
  </svg>
);

export const HomeIcon = (props: IconProps) => (
  <Svg {...props}>
    <path
      d="M3 10.6 12 3.2l9 7.4V20a1 1 0 0 1-1 1h-5v-6.2H9V21H4a1 1 0 0 1-1-1z"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinejoin="round"
    />
  </Svg>
);

export const IncidentIcon = (props: IconProps) => (
  <Svg {...props}>
    <path
      d="M4 4h16v13H8l-4 4zM8.5 8.5h7M8.5 12.5h4"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </Svg>
);

export const EnvironmentIcon = (props: IconProps) => (
  <Svg {...props}>
    <rect x="3" y="4" width="18" height="6" rx="1.4" stroke="currentColor" strokeWidth="1.7" />
    <rect x="3" y="14" width="18" height="6" rx="1.4" stroke="currentColor" strokeWidth="1.7" />
    <path d="M6.6 7h.01M6.6 17h.01" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" />
  </Svg>
);

export const ObservabilityIcon = (props: IconProps) => (
  <Svg {...props}>
    <path
      d="M3 3v18h18M7 15.5l4-5.5 3 3 5-7.5"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </Svg>
);

export const CorrelationIcon = (props: IconProps) => (
  <Svg {...props}>
    <path
      d="M12 3.2 13.9 8.1 18.8 10 13.9 11.9 12 16.8 10.1 11.9 5.2 10l4.9-1.9zM18.5 15.5l.8 2 2 .8-2 .8-.8 2-.8-2-2-.8 2-.8z"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinejoin="round"
    />
  </Svg>
);

export const TopologyIcon = (props: IconProps) => (
  <Svg {...props}>
    <path
      d="M12 2.5h4v4h-4zM3 17.5h4v4H3zM17 17.5h4v4h-4zM14 6.5v3H5v8M14 9.5h5v8"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinejoin="round"
    />
  </Svg>
);

export const ChangesIcon = (props: IconProps) => (
  <Svg {...props}>
    <path
      d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8zM14 3v5h5M9 13h6M9 17h4"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinejoin="round"
    />
  </Svg>
);

export const VulnerabilityIcon = (props: IconProps) => (
  <Svg {...props}>
    <path
      d="M12 3.5 20 7v6c0 4.5-3.2 7.5-8 9-4.8-1.5-8-4.5-8-9V7z"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinejoin="round"
    />
  </Svg>
);

export const AutomationsIcon = (props: IconProps) => (
  <Svg {...props}>
    <path
      d="M5 4.5h11a3 3 0 0 1 3 3V21H8a3 3 0 0 1-3-3zM19 21a3 3 0 0 1-3-3V4.5M9 9h6M9 13h4"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinejoin="round"
    />
  </Svg>
);

export const IntegrationsIcon = (props: IconProps) => (
  <Svg {...props}>
    <path
      d="M9 3v4M15 3v4M6 7h12v4a6 6 0 0 1-12 0zM9 17v4M15 17v4"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </Svg>
);

export const PoliciesIcon = (props: IconProps) => (
  <Svg {...props}>
    <path
      d="M6 2.5h9l3 3V21H6zM15 2.5V6h3M9 12.5l2 2 4-4.5"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinejoin="round"
    />
  </Svg>
);

export const AuditIcon = (props: IconProps) => (
  <Svg {...props}>
    <path
      d="M4 4.5h16v15H4zM7.5 9.5l2.5 2.5-2.5 2.5M13 15h4"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </Svg>
);

export const SearchIcon = (props: IconProps) => (
  <Svg {...props}>
    <circle cx="11" cy="11" r="7" stroke="currentColor" strokeWidth="1.8" />
    <path d="m16.5 16.5 4 4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
  </Svg>
);

export const BellIcon = (props: IconProps) => (
  <Svg {...props}>
    <path
      d="M6.5 9a5.5 5.5 0 0 1 11 0c0 6 2.5 6.5 2.5 8.5H4c0-2 2.5-2.5 2.5-8.5M10 21a2.2 2.2 0 0 0 4 0"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </Svg>
);

export const TerminalIcon = (props: IconProps) => (
  <Svg {...props}>
    <rect x="3" y="4" width="18" height="16" rx="1.6" stroke="currentColor" strokeWidth="1.6" />
    <path
      d="m7 9 3.5 3L7 15M13 15h4"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
    />
  </Svg>
);

export const LanguageIcon = (props: IconProps) => (
  <Svg {...props}>
    <circle cx="12" cy="12" r="9" stroke="currentColor" strokeWidth="1.6" />
    <path
      d="M3 12h18M12 3c2.6 2.6 2.6 15 0 18M12 3c-2.6 2.6-2.6 15 0 18"
      stroke="currentColor"
      strokeWidth="1.6"
    />
  </Svg>
);

export const ChevronIcon = ({
  direction = "right",
  ...props
}: IconProps & { direction?: "left" | "right" }) => (
  <Svg {...props}>
    <path
      d={direction === "left" ? "m15 5-7 7 7 7" : "m9 5 7 7-7 7"}
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </Svg>
);
