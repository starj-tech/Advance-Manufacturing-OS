import { Routes, Route, Navigate } from 'react-router-dom';
import { TasksPage } from './TasksPage';
import { SosPage } from './SosPage';
import { ClockPage } from './ClockPage';
import { EmployeeNav } from './EmployeeNav';

export default function EmployeeShell() {
  return (
    <div style={{ display: 'grid', gridTemplateRows: '1fr auto', height: '100%' }}>
      <main style={{ overflow: 'auto', padding: 16 }}>
        <Routes>
          <Route index element={<Navigate to="tasks" replace />} />
          <Route path="tasks" element={<TasksPage />} />
          <Route path="sos" element={<SosPage />} />
          <Route path="clock" element={<ClockPage />} />
        </Routes>
      </main>
      <EmployeeNav />
    </div>
  );
}
