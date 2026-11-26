-- Migration: period guard triggers (P0-4)

-- Trigger: block INSERT into vouchers when period is closed
CREATE TRIGGER IF NOT EXISTS trg_vouchers_before_insert
BEFORE INSERT ON vouchers
FOR EACH ROW
BEGIN
    SELECT CASE WHEN EXISTS (
        SELECT 1 FROM accounting_periods
        WHERE year = CAST(substr(NEW.voucher_date,1,4) AS INTEGER)
          AND month = CAST(substr(NEW.voucher_date,6,2) AS INTEGER)
          AND is_closed = 1
    ) THEN RAISE(ABORT, 'PERIOD_CLOSED: cannot insert voucher in closed period') END;
END;

-- Trigger: block INSERT into voucher_lines when parent voucher's period is closed
CREATE TRIGGER IF NOT EXISTS trg_voucher_lines_before_insert
BEFORE INSERT ON voucher_lines
FOR EACH ROW
BEGIN
    SELECT CASE WHEN EXISTS (
        SELECT 1 FROM vouchers v
        JOIN accounting_periods p ON p.year = CAST(substr(v.voucher_date,1,4) AS INTEGER)
                                AND p.month = CAST(substr(v.voucher_date,6,2) AS INTEGER)
                                AND p.is_closed = 1
        WHERE v.id = NEW.voucher_id
    ) THEN RAISE(ABORT, 'PERIOD_CLOSED: cannot insert voucher_line in closed period') END;
END;

-- Trigger: block status change (UPDATE) on vouchers when period is closed
CREATE TRIGGER IF NOT EXISTS trg_vouchers_before_update_status
BEFORE UPDATE ON vouchers
FOR EACH ROW WHEN OLD.status <> NEW.status
BEGIN
    SELECT CASE WHEN EXISTS (
        SELECT 1 FROM accounting_periods
        WHERE year = CAST(substr(OLD.voucher_date,1,4) AS INTEGER)
          AND month = CAST(substr(OLD.voucher_date,6,2) AS INTEGER)
          AND is_closed = 1
    ) THEN RAISE(ABORT, 'PERIOD_CLOSED: cannot change status of voucher in closed period') END;
END;
