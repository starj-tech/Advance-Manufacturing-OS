import { SessionProvider } from '@aether/auth';
import { ConsoleView } from './ConsoleView';

export function App() {
  return (
    <SessionProvider>
      <ConsoleView />
    </SessionProvider>
  );
}
