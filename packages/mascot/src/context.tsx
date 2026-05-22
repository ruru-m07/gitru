import {
  createContext,
  Dispatch,
  ReactNode,
  SetStateAction,
  useCallback,
  useState,
} from "react";
import type { MascotInteraction } from "./types";

export type MascotContextValue = {
  interaction: MascotInteraction;
  setInteraction: Dispatch<SetStateAction<MascotInteraction>>;
};

export type MascotProviderProps = {
  children: ReactNode;
  interaction?: MascotInteraction;
  defaultInteraction?: MascotInteraction;
  onInteractionChange?: (interaction: MascotInteraction) => void;
};

const MascotContext = createContext<MascotContextValue | null>(null);

const useControllableState = <T,>(
  controlled: T | undefined,
  defaultValue: T,
  onChange?: (value: T) => void,
) => {
  const [value, setValue] = useState(defaultValue);
  const isControlled = controlled !== undefined;
  const current = isControlled ? controlled : value;

  const set = useCallback(
    (next: SetStateAction<T>) => {
      const resolved =
        typeof next === "function" ? (next as (prev: T) => T)(current) : next;
      if (!isControlled) {
        setValue(resolved);
      }
      onChange?.(resolved);
    },
    [current, isControlled, onChange],
  );

  return [current, set] as const;
};

const MascotProvider = ({
  children,
  interaction,
  defaultInteraction = "idle",
  onInteractionChange,
}: MascotProviderProps) => {
  const [currentInteraction, setInteraction] = useControllableState(
    interaction,
    defaultInteraction,
    onInteractionChange,
  );

  return (
    <MascotContext.Provider
      value={{ interaction: currentInteraction, setInteraction }}
    >
      {children}
    </MascotContext.Provider>
  );
};

export { MascotContext, MascotProvider };
