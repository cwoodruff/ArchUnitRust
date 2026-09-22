use crate::inheritancecycle::slice2::TraitInSliceTwo;
use crate::inheritancecycle::slice3::TraitInSliceThree;

pub struct ClassInSliceOneImplementingTraitOfSliceTwo;

impl TraitInSliceOne for ClassInSliceOneImplementingTraitOfSliceTwo {}
impl TraitInSliceThree for ClassInSliceOneImplementingTraitOfSliceTwo {}
impl TraitInSliceTwo for ClassInSliceOneImplementingTraitOfSliceTwo {}

pub trait TraitInSliceOne {}
