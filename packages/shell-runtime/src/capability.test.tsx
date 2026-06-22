import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { CapabilityProvider, IndustryGate, useCapability } from './capability';

describe('useCapability', () => {
  it('returns true when the capability is in the granted set', () => {
    function Probe() {
      return <span>{useCapability('expired-date-tracking') ? 'yes' : 'no'}</span>;
    }
    render(
      <CapabilityProvider
        initialIndustrySlug="food-and-beverage"
        initialGranted={['expired-date-tracking', 'cold-chain-monitor']}
      >
        <Probe />
      </CapabilityProvider>,
    );
    expect(screen.getByText('yes')).toBeInTheDocument();
  });

  it('returns false when the capability is not granted', () => {
    function Probe() {
      return <span>{useCapability('parts-serial-tracking') ? 'yes' : 'no'}</span>;
    }
    render(
      <CapabilityProvider
        initialIndustrySlug="food-and-beverage"
        initialGranted={['expired-date-tracking']}
      >
        <Probe />
      </CapabilityProvider>,
    );
    expect(screen.getByText('no')).toBeInTheDocument();
  });

  it('honors explicit force-on override even when not in granted set', () => {
    function Probe() {
      return <span>{useCapability('andon-line-stop') ? 'yes' : 'no'}</span>;
    }
    render(
      <CapabilityProvider
        initialIndustrySlug="food-and-beverage"
        initialGranted={[]}
        initialOverrides={new Map([['andon-line-stop', true]])}
      >
        <Probe />
      </CapabilityProvider>,
    );
    expect(screen.getByText('yes')).toBeInTheDocument();
  });

  it('honors explicit force-off override even when in granted set', () => {
    function Probe() {
      return <span>{useCapability('expired-date-tracking') ? 'yes' : 'no'}</span>;
    }
    render(
      <CapabilityProvider
        initialIndustrySlug="food-and-beverage"
        initialGranted={['expired-date-tracking']}
        initialOverrides={new Map([['expired-date-tracking', false]])}
      >
        <Probe />
      </CapabilityProvider>,
    );
    expect(screen.getByText('no')).toBeInTheDocument();
  });

  it('throws when used outside a CapabilityProvider', () => {
    function Probe() {
      useCapability('anything');
      return null;
    }
    // Suppress the React error log noise during the negative test.
    const originalError = console.error;
    console.error = () => {};
    expect(() => render(<Probe />)).toThrow(/within <CapabilityProvider>/);
    console.error = originalError;
  });
});

describe('IndustryGate', () => {
  it('renders children when capability is granted', () => {
    render(
      <CapabilityProvider initialGranted={['x']}>
        <IndustryGate capability="x">
          <span>visible</span>
        </IndustryGate>
      </CapabilityProvider>,
    );
    expect(screen.getByText('visible')).toBeInTheDocument();
  });

  it('renders fallback when capability is not granted', () => {
    render(
      <CapabilityProvider initialGranted={[]}>
        <IndustryGate capability="x" fallback={<span>fallback</span>}>
          <span>visible</span>
        </IndustryGate>
      </CapabilityProvider>,
    );
    expect(screen.getByText('fallback')).toBeInTheDocument();
    expect(screen.queryByText('visible')).not.toBeInTheDocument();
  });
});
