//! 工作空间连接、身份、动态权限和按需业务模块状态。

use std::collections::BTreeMap;

use dioxus::prelude::SyncSignal;

use crate::spacetime_bindings::{
    access_car_type::AccessCar, access_visitor_type::AccessVisitor, amount_bill_type::AmountBill,
    bill_collection_confirmation_type::BillCollectionConfirmation,
    carryover_batch_type::CarryoverBatch, carryover_item_type::CarryoverItem,
    attendance_device_abnormal_log_type::AttendanceDeviceAbnormalLog,
    attendance_location_type::AttendanceLocation, attendance_type::Attendance,
    center_user_type::CenterUser, dashboard_overview_type::DashboardOverview,
    device_asset_image_preview_type::DeviceAssetImagePreview, device_asset_type::DeviceAsset,
    edge_gateway_type::EdgeGateway,
    dormitory_floor_type::DormitoryFloor,
    dormitory_image_preview_type::DormitoryImagePreview, dormitory_type::Dormitory,
    ele_bill_type::EleBill, elevator_asset_image_preview_type::ElevatorAssetImagePreview,
    elevator_asset_type::ElevatorAsset,
    elevator_inspection_image_preview_type::ElevatorInspectionImagePreview,
    elevator_inspection_type::ElevatorInspection, employee_type::Employee,
    employee_user_option_type::EmployeeUserOption,
    factory_floor_image_preview_type::FactoryFloorImagePreview, factory_floor_type::FactoryFloor,
    factory_type::Factory, finance_image_type::FinanceImage, finance_type::Finance,
    firefighting_asset_image_preview_type::FirefightingAssetImagePreview,
    firefighting_asset_type::FirefightingAsset,
    firefighting_inspection_image_preview_type::FirefightingInspectionImagePreview,
    firefighting_inspection_type::FirefightingInspection,
    leave_application_type::LeaveApplication, localization_type::Localization, menu_type::Menu,
    park_image_preview_type::ParkImagePreview, park_type::Park,
    reimbursement_image_preview_type::ReimbursementImagePreview, reimbursement_type::Reimbursement,
    rental_tenant_dormitory_floor_type::RentalTenantDormitoryFloor,
    rental_tenant_fee_type::RentalTenantFee, rental_tenant_floor_type::RentalTenantFloor,
    rental_tenant_meter_type::RentalTenantMeter,
    rental_tenant_type::RentalTenant,
    repair_order_type::RepairOrder, role_menu_type::RoleMenu,
    role_park_type::RolePark, role_type::Role, salary_image_preview_type::SalaryImagePreview,
    salary_type::Salary, system_user_type::SystemUser,
    tenant_image_preview_type::TenantImagePreview, tenant_profile_type::TenantProfile,
    transformer_asset_image_preview_type::TransformerAssetImagePreview,
    transformer_asset_type::TransformerAsset,
    transformer_inspection_image_preview_type::TransformerInspectionImagePreview,
    transformer_inspection_type::TransformerInspection,
    utility_meter_type::UtilityMeter,
    water_bill_type::WaterBill,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionPhase {
    Disconnected,
    Connecting,
    Syncing,
    Connected,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthState {
    Checking,
    SignedOut,
    SigningIn,
    SignedIn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DataScope {
    Core,
    Dashboard,
    /// 数据地图：跨模块的全局关系视图，订阅的是其他各域查询的并集。
    DataMap,
    Permissions,
    Rental,
    Maintenance,
    AccessControl,
    Salary,
    Billing,
    Finance,
    Reimbursements,
    Hr,
    /// 智能水电表管理：设备列表来自合众 HTTP 接口，但「这台设备绑给了哪块表」
    /// 要读我们自己的台账，所以这一页也需要订阅园区资产。
    SmartMeter,
    /// 设备管理：摄像头、门禁一类联网设备的台账。
    ///
    /// 与 `SmartMeter` 分开是因为读的表不同——那一页读水电表台账，这一页读
    /// `device_asset`，只有总览要把两者并排数一次。
    Device,
}

impl DataScope {
    pub fn for_path(path: &str) -> Option<Self> {
        match path {
            "/" => Some(Self::Dashboard),
            "/data-map" => Some(Self::DataMap),
            "/rental/salary" => Some(Self::Salary),
            value if value.starts_with("/rental/") => Some(Self::Rental),
            value if value == "/bill" || value.starts_with("/bill/") => Some(Self::Billing),
            value if value.starts_with("/finance/") => Some(Self::Finance),
            value if value.starts_with("/reimbursement/") => Some(Self::Reimbursements),
            value if value.starts_with("/hrm/") => Some(Self::Hr),
            value if value.starts_with("/maintenance/") => Some(Self::Maintenance),
            value if value.starts_with("/smart-meter/") => Some(Self::SmartMeter),
            value if value.starts_with("/device") => Some(Self::Device),
            value if value.starts_with("/access/") => Some(Self::AccessControl),
            value if value.starts_with("/system/") => Some(Self::Permissions),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Core => "账号与权限",
            Self::Dashboard => "运营总览",
            Self::DataMap => "数据地图",
            Self::Permissions => "权限管理",
            Self::Rental => "租赁资产",
            Self::Maintenance => "设施维护",
            Self::AccessControl => "门禁管理",
            Self::Salary => "工资管理",
            Self::Billing => "账单管理",
            Self::Finance => "财务管理",
            Self::Reimbursements => "报销管理",
            Self::Hr => "人力资源",
            Self::SmartMeter => "智能水电表管理",
            Self::Device => "设备管理",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModuleLoadState {
    Idle,
    Loading,
    Ready,
    Error(String),
}

impl ConnectionPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Disconnected => "连接已断开",
            Self::Connecting => "正在建立连接",
            Self::Syncing => "正在同步权限",
            Self::Connected => "实时连接",
            Self::Failed => "连接异常",
        }
    }

    pub fn class_name(self) -> &'static str {
        match self {
            Self::Disconnected => "is-idle",
            Self::Connecting | Self::Syncing => "is-pending",
            Self::Connected => "is-online",
            Self::Failed => "is-error",
        }
    }
}

#[derive(Clone, Copy)]
pub struct WorkspaceState {
    pub phase: SyncSignal<ConnectionPhase>,
    pub auth_state: SyncSignal<AuthState>,
    pub active_scope: SyncSignal<Option<DataScope>>,
    pub module_states: SyncSignal<BTreeMap<DataScope, ModuleLoadState>>,
    pub current_user: SyncSignal<Option<CenterUser>>,
    pub business_user: SyncSignal<Option<SystemUser>>,
    pub menus: SyncSignal<Vec<Menu>>,
    pub roles: SyncSignal<Vec<Role>>,
    /// 当前账号持有的权限码，用于与服务端一致地判定职能。
    pub permission_codes: SyncSignal<Vec<String>>,
    /// 权限管理员可维护的完整角色、菜单和园区策略数据。
    pub permission_roles: SyncSignal<Vec<Role>>,
    pub permission_menus: SyncSignal<Vec<Menu>>,
    pub permission_parks: SyncSignal<Vec<Park>>,
    pub permission_role_menus: SyncSignal<Vec<RoleMenu>>,
    pub permission_role_parks: SyncSignal<Vec<RolePark>>,
    pub dashboard_overview: SyncSignal<Option<DashboardOverview>>,
    pub rental_tenants: SyncSignal<Vec<RentalTenant>>,
    /// 合同租用了哪些楼层。多对多——一份合同可跨多层，一层可分租给多份合同。
    pub rental_tenant_floors: SyncSignal<Vec<RentalTenantFloor>>,
    pub tenant_profiles: SyncSignal<Vec<TenantProfile>>,
    pub parks: SyncSignal<Vec<Park>>,
    pub park_image_previews: SyncSignal<Vec<ParkImagePreview>>,
    pub factories: SyncSignal<Vec<Factory>>,
    pub factory_floors: SyncSignal<Vec<FactoryFloor>>,
    pub factory_floor_image_previews: SyncSignal<Vec<FactoryFloorImagePreview>>,
    pub dormitories: SyncSignal<Vec<Dormitory>>,
    /// 宿舍的逐层记录。层高、挂牌租金、房间数都在这里，不再压在宿舍那一行。
    pub dormitory_floors: SyncSignal<Vec<DormitoryFloor>>,
    /// 合同占用了哪些宿舍楼层、各占几间。"某层已用" 由它算出，不再人工填。
    pub rental_tenant_dormitory_floors: SyncSignal<Vec<RentalTenantDormitoryFloor>>,
    pub dormitory_image_previews: SyncSignal<Vec<DormitoryImagePreview>>,
    /// 水电表台账。表是资产，安装在某个楼层或园区公共区域，不随租户更替消失。
    pub utility_meters: SyncSignal<Vec<UtilityMeter>>,
    /// 设备台账：摄像头、门禁一体机、道闸、闸机。水电表**不在这里**，它们留在
    /// `utility_meters`——那张表还挂着倍率与账单引用，合并等于动账。
    pub device_assets: SyncSignal<Vec<DeviceAsset>>,
    pub device_asset_image_previews: SyncSignal<Vec<DeviceAssetImagePreview>>,
    /// 边缘计算设备台账。在线状态由连接生死翻转，不是心跳。
    pub edge_gateways: SyncSignal<Vec<EdgeGateway>>,
    /// 合同用了哪几块表、各自约定什么单价。水电定价是合同条款，不是开账单时的临时输入。
    pub rental_tenant_meters: SyncSignal<Vec<RentalTenantMeter>>,
    pub rental_tenant_fees: SyncSignal<Vec<RentalTenantFee>>,
    /// 消防设施资产台账。一行一个设施；巡检历史在 `firefighting_inspections`。
    pub firefighting_assets: SyncSignal<Vec<FirefightingAsset>>,
    /// 消防设施巡检记录，只增不删。
    pub firefighting_inspections: SyncSignal<Vec<FirefightingInspection>>,
    pub firefighting_asset_image_previews: SyncSignal<Vec<FirefightingAssetImagePreview>>,
    pub firefighting_inspection_image_previews:
        SyncSignal<Vec<FirefightingInspectionImagePreview>>,
    /// 变压器资产台账。一行一台真实设备；巡检历史在 `transformer_inspections`。
    pub transformer_assets: SyncSignal<Vec<TransformerAsset>>,
    /// 变压器巡检记录，只增不删。
    pub transformer_inspections: SyncSignal<Vec<TransformerInspection>>,
    pub transformer_asset_image_previews: SyncSignal<Vec<TransformerAssetImagePreview>>,
    pub transformer_inspection_image_previews: SyncSignal<Vec<TransformerInspectionImagePreview>>,
    /// 电梯资产台账。一行一台真实设备；巡检历史在 `elevator_inspections`。
    pub elevator_assets: SyncSignal<Vec<ElevatorAsset>>,
    /// 电梯巡检记录，只增不删。
    pub elevator_inspections: SyncSignal<Vec<ElevatorInspection>>,
    pub elevator_asset_image_previews: SyncSignal<Vec<ElevatorAssetImagePreview>>,
    pub elevator_inspection_image_previews: SyncSignal<Vec<ElevatorInspectionImagePreview>>,
    pub repair_orders: SyncSignal<Vec<RepairOrder>>,
    pub access_cars: SyncSignal<Vec<AccessCar>>,
    pub access_visitors: SyncSignal<Vec<AccessVisitor>>,
    pub salaries: SyncSignal<Vec<Salary>>,
    pub salary_image_previews: SyncSignal<Vec<SalaryImagePreview>>,
    pub reimbursements: SyncSignal<Vec<Reimbursement>>,
    pub reimbursement_image_previews: SyncSignal<Vec<ReimbursementImagePreview>>,
    pub employees: SyncSignal<Vec<Employee>>,
    pub employee_user_options: SyncSignal<Vec<EmployeeUserOption>>,
    pub attendances: SyncSignal<Vec<Attendance>>,
    pub localizations: SyncSignal<Vec<Localization>>,
    pub leave_applications: SyncSignal<Vec<LeaveApplication>>,
    pub attendance_locations: SyncSignal<Vec<AttendanceLocation>>,
    pub attendance_abnormal_logs: SyncSignal<Vec<AttendanceDeviceAbnormalLog>>,
    pub tenant_image_previews: SyncSignal<Vec<TenantImagePreview>>,
    pub amount_bills: SyncSignal<Vec<AmountBill>>,
    /// 收缴确认记录，只增不删。收缴对账状态由「账单金额 + 这张表」现场推导。
    pub bill_collection_confirmations: SyncSignal<Vec<BillCollectionConfirmation>>,
    /// 账期结转批次。系统算出、经理确认后差额才并入下期。
    pub carryover_batches: SyncSignal<Vec<CarryoverBatch>>,
    pub carryover_items: SyncSignal<Vec<CarryoverItem>>,
    pub finances: SyncSignal<Vec<Finance>>,
    pub finance_images: SyncSignal<Vec<FinanceImage>>,
    pub ele_bills: SyncSignal<Vec<EleBill>>,
    pub water_bills: SyncSignal<Vec<WaterBill>>,
    pub identity: SyncSignal<Option<String>>,
    pub authenticating: SyncSignal<bool>,
    pub error_message: SyncSignal<Option<String>>,
    pub last_synced_label: SyncSignal<String>,
}
