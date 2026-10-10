import { useRoute } from "./route.ts";
import { InstancesPage } from "./pages/InstancesPage.tsx";
import { NowPage } from "./pages/NowPage.tsx";
import { TurnsPage } from "./pages/TurnsPage.tsx";

export function App() {
  const route = useRoute();
  if (route.page === "now") return <NowPage id={route.id} />;
  if (route.page === "turns") return <TurnsPage id={route.id} turn={route.turn} />;
  return <InstancesPage />;
}
