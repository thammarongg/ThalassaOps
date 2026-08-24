const en = {
  health: {
    eyebrow: "ThalassaOps local shell",
    title: "System health",
    checking: "Checking local core…",
    error: "Health check failed",
    policyVersion: "Policy version"
  },
  status: {
    healthy: "Healthy",
    degraded: "Degraded",
    unavailable: "Unavailable",
    warning: "Warning",
    critical: "Critical"
  },
  severity: {
    s1: "S1 Critical",
    s2: "S2 Major",
    s3: "S3 Moderate",
    s4: "S4 Minor",
    s5: "S5 Informational"
  },
  demo: {
    title: "Design system preview",
    primaryCard: "Operational state",
    secondaryCard: "Reusable empty state",
    emptyTitle: "No evidence selected",
    tableCaption: "Example environment data",
    name: "Name",
    firstTab: "Overview",
    secondTab: "Evidence",
    timelineEvent: "Signal received",
    commandLabel: "Command surface",
    commandPlaceholder: "Search commands",
    drawerTitle: "Component drawer",
    close: "Close",
    timelineTitle: "Evidence tide line",
    healthCard: "Core status"
  }
} as const;

export default en;
