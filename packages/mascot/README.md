# Gitru's mascot

## Quick use

```tsx
import { Mascot } from "@gitru/mascot";

export const Example = () => <Mascot />;
```

## Expressions and interactions

Control the expression directly:

```tsx
import { Mascot } from "@gitru/mascot";

export const Example = () => (
	<Mascot expression={{ eyes: "closed", mouth: "open" }} />
);
```

Invert the default hover behavior:

```tsx
import { Mascot } from "@gitru/mascot";

export const Example = () => (
	<Mascot
		expressionMap={{
			idle: { eyes: "closed", mouth: "open" },
			hover: { eyes: "open", mouth: "neutral" },
		}}
	/>
);
```

Control interaction from outside:

```tsx
import { Mascot } from "@gitru/mascot";
import { useState } from "react";

export const Example = () => {
	const [interaction, setInteraction] = useState<
		"idle" | "hover" | "press" | "focus"
	>("idle");

	return (
		<Mascot
			interaction={interaction}
			onInteractionChange={setInteraction}
			behavior={{ hover: false, press: false, focus: false }}
		/>
	);
};
```

## Particles

```tsx
import { Mascot } from "@gitru/mascot";

export const Example = () => (
	<Mascot
		particles={{
			enabled: true,
			count: 10,
			sizeRange: [24, 48],
			offset: { x: 0.5, y: 0.2 },
		}}
	/>
);
```

## Shared state

```tsx
import { Mascot, MascotProvider, useMascot } from "@gitru/mascot";

const Controls = () => {
	const { setInteraction } = useMascot();

	return (
		<button onClick={() => setInteraction("hover")}>
			Force hover
		</button>
	);
};

export const Example = () => (
	<MascotProvider>
		<Mascot />
		<Mascot />
		<Controls />
	</MascotProvider>
);
```
