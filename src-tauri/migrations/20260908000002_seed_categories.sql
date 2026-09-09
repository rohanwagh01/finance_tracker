-- Top-level spending categories, aligned with Plaid's Personal Finance Category
-- taxonomy (primary tier). Detailed subcategories are created on the fly during
-- sync with parent_id pointing at one of these.

INSERT INTO categories (id, parent_id, label, is_custom) VALUES
    ('INCOME',                    NULL, 'Income',                    0),
    ('TRANSFER_IN',               NULL, 'Transfer In',               0),
    ('TRANSFER_OUT',              NULL, 'Transfer Out',              0),
    ('LOAN_PAYMENTS',             NULL, 'Loan Payments',             0),
    ('BANK_FEES',                 NULL, 'Bank Fees',                 0),
    ('ENTERTAINMENT',             NULL, 'Entertainment',             0),
    ('FOOD_AND_DRINK',            NULL, 'Food & Drink',              0),
    ('GENERAL_MERCHANDISE',       NULL, 'Shopping',                  0),
    ('HOME_IMPROVEMENT',          NULL, 'Home Improvement',          0),
    ('MEDICAL',                   NULL, 'Medical',                   0),
    ('PERSONAL_CARE',             NULL, 'Personal Care',             0),
    ('GENERAL_SERVICES',          NULL, 'Services',                  0),
    ('GOVERNMENT_AND_NON_PROFIT', NULL, 'Government & Non-Profit',   0),
    ('TRANSPORTATION',            NULL, 'Transportation',            0),
    ('TRAVEL',                    NULL, 'Travel',                    0),
    ('RENT_AND_UTILITIES',        NULL, 'Rent & Utilities',          0),
    ('OTHER',                     NULL, 'Uncategorized',             0);
