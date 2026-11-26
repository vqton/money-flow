-- Migration: Add period guard triggers (P0-4)

-- 1. Block INSERT voucher when period is closed
CREATE TRIGGER IF NOT EXISTS trg_vouchers_before_insert_closed
BEFORE INSERT ON vouchers
FOR EACH ROW
WHEN (SELECT is_closed
      FROM accounting_periods
      WHERE year = CAST(substr(NEW.voucher_date,1,4) AS INTEGER)
        AND month = CAST(substr(NEW.voucher_date,6,2) AS INTEGER)) = 1
BEGIN
    SELECT RAISE(ABORT, 'PERIOD_CLOSED: cannot insert voucher into a closed period');
END;

-- 2. Block INSERT voucher_line when its parent voucher belongs to a closed period
CREATE TRIGGER IF NOT EXISTS trg_voucher_lines_before_insert_closed
BEFORE INSERT ON voucher_lines
FOR EACH ROW
WHEN (SELECT is_closed
      FROM accounting_periods
      WHERE year = CAST(substr(
                (SELECT voucher_date FROM vouchers WHERE id = NEW.voucher_id),
                1,4) AS INTEGER)
        AND month = CAST(substr(
                (SELECT voucher_date FROM vouchers WHERE id = NEW.voucher_id),
                6,2) AS INTEGER)) = 1
BEGIN
    SELECT RAISE(ABORT, 'PERIOD_CLOSED: cannot insert voucher line into a closed period');
END;

-- 3. Block VOID (POSTED->VOIDED) when period is closed
CREATE TRIGGER IF NOT EXISTS trg_vouchers_before_update_void_closed
BEFORE UPDATE ON vouchers
FOR EACH ROW
WHEN OLD.status = 'POSTED' AND NEW.status = 'VOIDED'
     AND (SELECT is_closed
          FROM accounting_periods
          WHERE year = CAST(substr(OLD.voucher_date,1,4) AS INTEGER)
            AND month = CAST(substr(OLD.voucher_date,6,2) AS INTEGER)) = 1
BEGIN
    SELECT RAISE(ABORT, 'PERIOD_CLOSED: cannot void voucher in a closed period');
END;
