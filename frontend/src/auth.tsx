import { createContext, useContext } from "react";

export interface AuthState {
  username?: string;
  csrf?: string;
  login: (username: string, csrf: string) => void;
  logout: () => Promise<void>;
}

export const AuthContext = createContext<AuthState>({
  login: () => {},
  logout: async () => {},
});

export const useAuth = () => useContext(AuthContext);
