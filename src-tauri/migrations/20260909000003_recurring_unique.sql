-- Detection should never create two rows for the same merchant + cadence.
CREATE UNIQUE INDEX idx_recurring_detected
    ON recurring_payments(detected_merchant, cadence)
    WHERE detected_merchant IS NOT NULL;
