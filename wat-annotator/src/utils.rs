use wast::{
    component::{ComponentField, ComponentKind},
    core::ModuleField,
    token::Span,
    Wat,
};

pub fn get_span(f: &ComponentField) -> Option<Span> {
    match f {
        ComponentField::CoreModule(cm) => Some(cm.span),
        ComponentField::CoreInstance(ci) => Some(ci.span),
        ComponentField::CoreType(ct) => Some(ct.span),
        ComponentField::Component(nc) => Some(nc.span),
        ComponentField::Instance(i) => Some(i.span),
        ComponentField::Alias(a) => Some(a.span),
        ComponentField::Type(t) => Some(t.span),
        ComponentField::CanonicalFunc(cf) => Some(cf.span),
        ComponentField::CoreFunc(cf) => Some(cf.span),
        ComponentField::Func(f) => Some(f.span),
        ComponentField::Start(_s) => None,
        ComponentField::Import(ci) => Some(ci.span),
        ComponentField::Export(ce) => Some(ce.span),
        ComponentField::Custom(c) => Some(c.span),
        ComponentField::Producers(_p) => None,
    }
}

pub fn get_module_span(f: &ModuleField) -> Option<Span> {
    match f {
        ModuleField::Type(f) => Some(f.span),
        ModuleField::Rec(f) => Some(f.span),
        ModuleField::Import(f) => Some(f.span),
        ModuleField::Func(f) => Some(f.span),
        ModuleField::Table(f) => Some(f.span),
        ModuleField::Memory(f) => Some(f.span),
        ModuleField::Global(f) => Some(f.span),
        ModuleField::Export(f) => Some(f.span),
        ModuleField::Start(_) => None,
        ModuleField::Elem(f) => Some(f.span),
        ModuleField::Data(f) => Some(f.span),
        ModuleField::Tag(f) => Some(f.span),
        ModuleField::Custom(_) => None,
    }
}

pub fn get_fields<'a, 'b: 'a>(comp: &'b Wat<'a>) -> Option<&'a Vec<ComponentField<'a>>> {
    match comp {
        Wat::Module(_) => None,
        Wat::Component(comp) => match &comp.kind {
            ComponentKind::Binary(_) => None,
            ComponentKind::Text(v) => Some(v),
        },
    }
}
