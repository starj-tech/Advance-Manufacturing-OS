import { Placeholder } from '../_shared/Placeholder';

export function InventoryPage() {
  return (
    <Placeholder
      title="Inventory"
      description="Live qty_on_hand. Adjustments flow through server RPC inventory_adjust(delta) to keep stock ≥ 0 atomic."
      shipsIn="PR #2 — sync engine"
    />
  );
}
