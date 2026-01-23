-- Ensure users table exists (defensive for fresh test DBs)
CREATE TABLE IF NOT EXISTS users (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    email VARCHAR(100) NOT NULL UNIQUE,
    password TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
);

-- Remove duplicate users keeping the earliest id for each email
DELETE u1 FROM users u1
INNER JOIN users u2
    ON u1.email = u2.email
    AND u1.id > u2.id;

-- Add unique index on email to prevent future duplicates (create only if missing)
-- Using INFORMATION_SCHEMA to check for existing index and CREATE via prepared statement
SET @exists := (
    SELECT COUNT(*)
    FROM INFORMATION_SCHEMA.STATISTICS
    WHERE TABLE_SCHEMA = DATABASE()
      AND TABLE_NAME = 'users'
      AND INDEX_NAME = 'ux_users_email'
);
SET @stmt := IF(@exists = 0,
    'ALTER TABLE users ADD UNIQUE INDEX ux_users_email (email);',
    'SELECT "index already exists";'
);
PREPARE stmt FROM @stmt;
EXECUTE stmt;
DEALLOCATE PREPARE stmt;
