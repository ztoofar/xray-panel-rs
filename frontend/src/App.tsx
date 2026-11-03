import { useEffect, useState } from "react";
import { Navigate, Route, Routes } from "react-router-dom";
import { Spin } from "antd";
import { api, post, setCsrf } from "./api";
import { AuthContext } from "./auth";
import AppLayout from "./layout/AppLayout";
import LoginPage from "./pages/Login";
import DashboardPage from "./pages/Dashboard";
import InboundsPage from "./pages/Inbounds";
import ClientsPage from "./pages/Clients";
import OutboundsPage from "./pages/Outbounds";
import RoutingPage from "./pages/Routing";
import XrayPage from "./pages/Xray";
import SettingsPage from "./pages/Settings";

interface Session {
  authenticated: boolean;
  username?: string;
  csrf?: string;
}

export default function App() {
  const [loading, setLoading] = useState(true);
  const [username, setUsername] = useState<string>();
  const [csrf, setCsrfState] = useState<string>();

  useEffect(() => {
    api<Session>("/api/session")
      .then((session) => {
        if (session.authenticated) {
          setCsrf(session.csrf ?? "");
          setCsrfState(session.csrf);
          setUsername(session.username);
        }
      })
      .catch(() => {})
      .finally(() => setLoading(false));
  }, []);

  if (loading) {
    return (
      <div className="boot-screen">
        <Spin size="large" />
      </div>
    );
  }

  const login = (user: string, token: string) => {
    setCsrf(token);
    setCsrfState(token);
    setUsername(user);
  };

  const logout = async () => {
    try {
      await post("/api/logout");
    } catch {
      // Session may already be gone; local state is cleared either way.
    }
    setCsrf("");
    setCsrfState(undefined);
    setUsername(undefined);
  };

  return (
    <AuthContext.Provider value={{ username, csrf, login, logout }}>
      <Routes>
        <Route
          path="/login"
          element={username ? <Navigate to="/" replace /> : <LoginPage />}
        />
        <Route
          path="/"
          element={username ? <AppLayout /> : <Navigate to="/login" replace />}
        >
          <Route index element={<DashboardPage />} />
          <Route path="inbounds" element={<InboundsPage />} />
          <Route path="clients" element={<ClientsPage />} />
          <Route path="outbound" element={<OutboundsPage />} />
          <Route path="routing" element={<RoutingPage />} />
          <Route path="xray" element={<XrayPage />} />
          <Route path="settings" element={<SettingsPage />} />
        </Route>
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </AuthContext.Provider>
  );
}
