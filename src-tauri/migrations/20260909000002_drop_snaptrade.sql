-- SnapTrade was replaced by Plaid Investments. Remove any leftover rows.
DELETE FROM items WHERE provider = 'snaptrade';
DELETE FROM app_config WHERE key IN ('snaptrade_user_id');
