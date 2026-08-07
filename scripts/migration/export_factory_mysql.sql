SELECT REPLACE(TO_BASE64(CAST(JSON_OBJECT(
  'factories', COALESCE((SELECT JSON_ARRAYAGG(JSON_OBJECT(
    'factory_id', factory_id,
    'factory_name', factory_name,
    'park_id', COALESCE(park_id, 0),
    'build_date', IF(build_time IS NULL, NULL, DATE_FORMAT(build_time, '%Y-%m-%d')),
    'address', address,
    'contact', contact,
    'description', description,
    'is_own', JSON_EXTRACT(IF(is_own, 'true', 'false'), '$'),
    'is_deleted', JSON_EXTRACT(IF(is_deleted, 'true', 'false'), '$'),
    'created_at_micros', IF(create_time IS NULL, NULL, CAST(UNIX_TIMESTAMP(create_time) * 1000000 AS SIGNED)),
    'updated_at_micros', IF(update_time IS NULL, NULL, CAST(UNIX_TIMESTAMP(update_time) * 1000000 AS SIGNED))
  )) FROM factory), JSON_ARRAY()),
  'floors', COALESCE((SELECT JSON_ARRAYAGG(JSON_OBJECT(
    'floor_id', floor_id,
    'floor_name', floor_name,
    'factory_id', factory_id,
    'floor_height_centi_metres', IF(floor_height IS NULL, NULL, CAST(ROUND(floor_height * 100) AS SIGNED)),
    'load_bearing_centi_units', IF(load_bearing IS NULL, NULL, CAST(ROUND(load_bearing * 100) AS SIGNED)),
    'rent_price_cents', CAST(ROUND(rent_price * 100) AS SIGNED),
    'total_area_centi_square_metres', CAST(ROUND(total_area * 100) AS SIGNED),
    'used_area_centi_square_metres', CAST(ROUND(used_area * 100) AS SIGNED),
    'status', status,
    'description', description,
    'is_deleted', JSON_EXTRACT(IF(is_deleted, 'true', 'false'), '$'),
    'created_at_micros', IF(create_time IS NULL, NULL, CAST(UNIX_TIMESTAMP(create_time) * 1000000 AS SIGNED)),
    'updated_at_micros', IF(update_time IS NULL, NULL, CAST(UNIX_TIMESTAMP(update_time) * 1000000 AS SIGNED))
  )) FROM factory_floor), JSON_ARRAY())
) AS CHAR)), '\n', '');
