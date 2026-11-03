import { Suspense } from 'react';
import { createBrowserRouter, type RouteObject } from 'react-router';
import { Spin } from 'antd';

import PanelLayout from '@/layouts/PanelLayout';
import { PANEL_ROUTES } from '@/routes/panelRoutes';

function withSuspense(node: React.ReactNode) {
  return (
    <Suspense
      fallback={
        <div
          style={{
            display: 'flex',
            justifyContent: 'center',
            alignItems: 'center',
            minHeight: '60vh',
          }}
        >
          <Spin size="large" />
        </div>
      }
    >
      {node}
    </Suspense>
  );
}

const DashboardPage = PANEL_ROUTES.find((route) => route.path === '/')?.component;

export const routes: RouteObject[] = [
  {
    path: '/',
    element: <PanelLayout />,
    children: [
      {
        index: true,
        element: DashboardPage ? withSuspense(<DashboardPage />) : null,
      },
      ...PANEL_ROUTES.filter((route) => route.path !== '/').map((route) => ({
        path: route.path.slice(1),
        element: withSuspense(<route.component />),
      })),
    ],
  },
];

function computeBasename() {
  const raw = (typeof window !== 'undefined' && window.X_UI_BASE_PATH) || '/';
  const trimmed = raw.replace(/\/+$/, '');
  return `${trimmed}/panel`;
}

export const router = createBrowserRouter(routes, {
  basename: computeBasename(),
});
