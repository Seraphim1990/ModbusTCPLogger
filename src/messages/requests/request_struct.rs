
use crate::messages::requests::{
    node_request::NodeRequest,
    device_request::DeviceRequest,
    value_request::ValueRequest,
    measure_request::MeasureRequest,
    user_request::UserRequest,
    group_request::GroupRequest,
    sub_group_request::SubGroupsRequest,
};
use crate::messages::requests::tokens_request::TokensRequest;

pub enum Request{
    GetNode(NodeRequest),
    GetDevice(DeviceRequest),
    GetValue(ValueRequest),
    GetMeasure(MeasureRequest),
    GetUser(UserRequest),
    GetGroup(GroupRequest),
    GetSubGroup(SubGroupsRequest),
    GetToken(TokensRequest),
}