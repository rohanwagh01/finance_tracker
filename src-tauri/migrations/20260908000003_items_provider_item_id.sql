-- Plaid's own item_id, used to dedupe re-links of the same institution login.
ALTER TABLE items ADD COLUMN provider_item_id TEXT;
CREATE UNIQUE INDEX idx_items_provider_item_id
    ON items(provider, provider_item_id)
    WHERE provider_item_id IS NOT NULL;
