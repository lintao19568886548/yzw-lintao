//! 工资记录与图片的多对多关系。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = salary_image,
    index(accessor = salary_image_by_customer, btree(columns = [customer_id])),
    index(accessor = salary_image_by_salary, btree(columns = [salary_id])),
    index(accessor = salary_image_by_image, btree(columns = [img_id])),
    index(accessor = salary_image_by_pair, btree(columns = [salary_id, img_id]))
)]
pub struct SalaryImage {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub customer_id: String,
    pub salary_id: u64,
    pub img_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
