import '@testing-library/jest-dom/vitest';

// Suppress noisy ResizeObserver errors in tests
const originalError = console.error;
console.error = (...args: unknown[]) => {
  if (typeof args[0] === 'string' && args[0].includes('ResizeObserver')) {
    return;
  }
  originalError(...args);
};
