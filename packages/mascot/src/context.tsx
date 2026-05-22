import {
  createContext,
  Dispatch,
  ReactNode,
  SetStateAction,
  useState,
} from "react";

export type MascotContextValue = {
  isHovered: boolean;
  setIsHovered: Dispatch<SetStateAction<boolean>>;
};

export type MascotProviderProps = {
  children: ReactNode;
};

const MascotContext = createContext<MascotContextValue | null>(null);

const MascotProvider = ({ children }: MascotProviderProps) => {
  const [isHovered, setIsHovered] = useState(false);

  return (
    <MascotContext.Provider value={{ isHovered, setIsHovered }}>
      {children}
    </MascotContext.Provider>
  );
};

export { MascotContext, MascotProvider };
