SELECT REPLACE(TO_BASE64(CAST(JSON_OBJECT(
  'parks', (SELECT JSON_ARRAYAGG(JSON_OBJECT(
    'park_id', park_id,
    'park_name', park_name,
    'address', address,
    'area_centi_square_metres', CAST(ROUND(area * 100) AS SIGNED),
    'description', description,
    'status', status,
    'contact', contact,
    'manager', manager,
    'is_deleted', is_deleted,
    'created_at_micros', IF(create_time IS NULL, NULL, CAST(UNIX_TIMESTAMP(create_time) * 1000000 AS SIGNED)),
    'updated_at_micros', IF(update_time IS NULL, NULL, CAST(UNIX_TIMESTAMP(update_time) * 1000000 AS SIGNED))
  )) FROM park),
  'rental_tenants', (SELECT JSON_ARRAYAGG(JSON_OBJECT(
    'rental_tenant_id', rental_tenant_id,
    'tenant_name', tenant_name,
    'phone_number', phone_number,
    'transaction_type', transaction_type,
    'status', status,
    'contract_start_micros', IF(contract_start IS NULL, NULL, CAST(UNIX_TIMESTAMP(contract_start) * 1000000 AS SIGNED)),
    'contract_end_micros', IF(contract_end IS NULL, NULL, CAST(UNIX_TIMESTAMP(contract_end) * 1000000 AS SIGNED)),
    'rental_amount_cents', IF(rental_amount IS NULL, NULL, CAST(ROUND(rental_amount * 100) AS SIGNED)),
    'increase_date_micros', IF(increase_date IS NULL, NULL, CAST(UNIX_TIMESTAMP(increase_date) * 1000000 AS SIGNED)),
    'increase_rate_basis_points', IF(increase_rate IS NULL, NULL, CAST(ROUND(increase_rate * 100) AS SIGNED)),
    'increase_data', increase_data,
    'penalty_rate_basis_points', IF(penalty_rate IS NULL, NULL, CAST(ROUND(penalty_rate * 100) AS SIGNED)),
    'area_centi_square_metres', IF(area IS NULL, NULL, CAST(ROUND(area * 100) AS SIGNED)),
    'address', address,
    'remark', remark,
    'park_id', COALESCE(park_id, 0),
    'send_message_at_micros', IF(send_message IS NULL, NULL, CAST(UNIX_TIMESTAMP(send_message) * 1000000 AS SIGNED)),
    'is_deleted', is_deleted,
    'created_at_micros', IF(create_time IS NULL, NULL, CAST(UNIX_TIMESTAMP(create_time) * 1000000 AS SIGNED)),
    'updated_at_micros', IF(update_time IS NULL, NULL, CAST(UNIX_TIMESTAMP(update_time) * 1000000 AS SIGNED))
  )) FROM rental_tenant),
  'salaries', (SELECT JSON_ARRAYAGG(JSON_OBJECT(
    'salary_id', salary_id,
    'rental_tenant_id', rental_tenant_id,
    'salary_amount_cents', IF(salary_amount IS NULL, NULL, CAST(ROUND(salary_amount * 100) AS SIGNED)),
    'issue_date_micros', IF(issue_date IS NULL, NULL, CAST(UNIX_TIMESTAMP(issue_date) * 1000000 AS SIGNED)),
    'issued', issued,
    'remark', remark,
    'is_deleted', is_deleted,
    'created_at_micros', IF(create_time IS NULL, NULL, CAST(UNIX_TIMESTAMP(create_time) * 1000000 AS SIGNED)),
    'updated_at_micros', IF(update_time IS NULL, NULL, CAST(UNIX_TIMESTAMP(update_time) * 1000000 AS SIGNED))
  )) FROM salary),
  'images', (SELECT JSON_ARRAYAGG(JSON_OBJECT(
    'img_id', img_id,
    'img_url', img_url,
    'hash', hash,
    'created_at_micros', IF(created_time IS NULL, NULL, CAST(UNIX_TIMESTAMP(created_time) * 1000000 AS SIGNED)),
    'updated_at_micros', IF(updated_time IS NULL, NULL, CAST(UNIX_TIMESTAMP(updated_time) * 1000000 AS SIGNED))
  )) FROM image WHERE img_id IN (SELECT img_id FROM salary_image)),
  'salary_images', (SELECT JSON_ARRAYAGG(JSON_OBJECT(
    'id', id,
    'salary_id', salary_id,
    'img_id', img_id,
    'created_at_micros', IF(create_time IS NULL, NULL, CAST(UNIX_TIMESTAMP(create_time) * 1000000 AS SIGNED)),
    'updated_at_micros', IF(update_time IS NULL, NULL, CAST(UNIX_TIMESTAMP(update_time) * 1000000 AS SIGNED))
  )) FROM salary_image)
) AS CHAR)), '\n', '');
