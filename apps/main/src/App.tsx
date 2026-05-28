import { SessionProvider } from '@aether/auth';
import { MainView } from './MainView';

export function App() {
  return (
    <SessionProvider>
      <MainView />
    </SessionProvider>
  );
}
