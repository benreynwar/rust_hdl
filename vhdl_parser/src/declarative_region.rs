// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2018, Olof Kraigher olof.kraigher@gmail.com
use ast::*;
use library::{EntityDesignUnit, Library, PackageDesignUnit};
use message::{Message, MessageHandler};
use source::{SrcPos, WithPos};
use symbol_table::Symbol;

extern crate fnv;
use self::fnv::FnvHashMap;
use std::collections::hash_map::Entry;

#[derive(PartialEq, Debug, Clone)]
pub enum AnyDeclaration<'a> {
    Declaration(&'a Declaration),
    Element(&'a ElementDeclaration),
    Enum(&'a WithPos<EnumerationLiteral>),
    Interface(&'a InterfaceDeclaration),
    Library(&'a Library<'a>),
    Package(&'a PackageDesignUnit<'a>),
    Context(&'a ContextDeclaration),
    Entity(&'a EntityDesignUnit),
    Configuration(&'a DesignUnit<ConfigurationDeclaration>),
    PackageInstance(&'a DesignUnit<PackageInstantiation>),
}

impl<'a> AnyDeclaration<'a> {
    fn is_deferred_constant(&self) -> bool {
        match self {
            AnyDeclaration::Declaration(Declaration::Object(ObjectDeclaration {
                ref class,
                ref expression,
                ..
            })) => *class == ObjectClass::Constant && expression.is_none(),
            _ => false,
        }
    }

    fn is_non_deferred_constant(&self) -> bool {
        match self {
            AnyDeclaration::Declaration(Declaration::Object(ObjectDeclaration {
                ref class,
                ref expression,
                ..
            })) => *class == ObjectClass::Constant && expression.is_some(),
            _ => false,
        }
    }

    fn is_protected_type(&self) -> bool {
        match self {
            AnyDeclaration::Declaration(Declaration::Type(TypeDeclaration {
                def: TypeDefinition::Protected { .. },
                ..
            })) => true,
            _ => false,
        }
    }

    fn is_protected_type_body(&self) -> bool {
        match self {
            AnyDeclaration::Declaration(Declaration::Type(TypeDeclaration {
                def: TypeDefinition::ProtectedBody { .. },
                ..
            })) => true,
            _ => false,
        }
    }

    fn is_incomplete_type(&self) -> bool {
        match self {
            AnyDeclaration::Declaration(Declaration::Type(TypeDeclaration {
                def: TypeDefinition::Incomplete,
                ..
            })) => true,
            _ => false,
        }
    }

    fn is_type_declaration(&self) -> bool {
        match self {
            AnyDeclaration::Declaration(Declaration::Type(..)) => true,
            _ => false,
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct VisibleDeclaration<'a> {
    pub designator: Designator,

    /// The location where the declaration was made
    /// Builtin and implicit declaration will not have a source position
    pub decl_pos: Option<SrcPos>,
    pub decl: AnyDeclaration<'a>,
    pub may_overload: bool,
}

impl<'a> VisibleDeclaration<'a> {
    pub fn new(
        designator: impl Into<WithPos<Designator>>,
        decl: AnyDeclaration<'a>,
    ) -> VisibleDeclaration<'a> {
        let designator = designator.into();
        VisibleDeclaration {
            designator: designator.item,
            decl_pos: Some(designator.pos),
            decl,
            may_overload: false,
        }
    }

    fn error(&self, messages: &mut MessageHandler, message: impl Into<String>) {
        if let Some(ref pos) = self.decl_pos {
            messages.push(Message::error(pos, message));
        }
    }

    fn hint(&self, messages: &mut MessageHandler, message: impl Into<String>) {
        if let Some(ref pos) = self.decl_pos {
            messages.push(Message::hint(pos, message));
        }
    }

    pub fn with_overload(mut self, value: bool) -> VisibleDeclaration<'a> {
        self.may_overload = value;
        self
    }

    fn is_deferred_of(&self, other: &Self) -> bool {
        (self.decl.is_deferred_constant() && other.decl.is_non_deferred_constant())
            || (self.decl.is_protected_type() && other.decl.is_protected_type_body())
            || (self.decl.is_incomplete_type()
                && other.decl.is_type_declaration()
                && !other.decl.is_incomplete_type())
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum RegionKind {
    PackageDeclaration,
    PackageBody,
    Other,
}

/// Most parent regions can just be temporarily borrowed
/// For public regions of design units the parent must be owned such that these regions can be stored in a map
#[derive(PartialEq, Debug, Clone)]
enum ParentRegion<'r, 'a: 'r> {
    Borrowed(&'r DeclarativeRegion<'r, 'a>),
    Owned(Box<DeclarativeRegion<'r, 'a>>),
}

impl<'r, 'a> std::ops::Deref for ParentRegion<'r, 'a> {
    type Target = DeclarativeRegion<'r, 'a>;

    fn deref(&self) -> &DeclarativeRegion<'r, 'a> {
        match self {
            ParentRegion::Borrowed(region) => region,
            ParentRegion::Owned(ref region) => region.as_ref(),
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct DeclarativeRegion<'r, 'a: 'r> {
    parent: Option<ParentRegion<'r, 'a>>,
    visible: FnvHashMap<Designator, VisibleDeclaration<'a>>,
    decls: FnvHashMap<Designator, VisibleDeclaration<'a>>,
    kind: RegionKind,
}

impl<'r, 'a: 'r> DeclarativeRegion<'r, 'a> {
    pub fn new(parent: Option<&'r DeclarativeRegion<'r, 'a>>) -> DeclarativeRegion<'r, 'a> {
        DeclarativeRegion {
            parent: parent.map(|parent| ParentRegion::Borrowed(parent)),
            visible: FnvHashMap::default(),
            decls: FnvHashMap::default(),
            kind: RegionKind::Other,
        }
    }

    pub fn in_package_declaration(mut self) -> DeclarativeRegion<'r, 'a> {
        self.kind = RegionKind::PackageDeclaration;
        self
    }

    /// Clone the region with owned version of all parents
    pub fn clone_owned_parent<'s>(&self) -> DeclarativeRegion<'s, 'a> {
        let parent = {
            if let Some(parent) = self.parent.as_ref() {
                Some(ParentRegion::Owned(Box::new(parent.clone_owned_parent())))
            } else {
                None
            }
        };

        DeclarativeRegion {
            parent,
            visible: self.visible.clone(),
            decls: self.decls.clone(),
            kind: self.kind,
        }
    }

    pub fn in_body(&self) -> DeclarativeRegion<'r, 'a> {
        let mut region = self.clone();
        region.kind = match region.kind {
            RegionKind::PackageDeclaration => RegionKind::PackageBody,
            _ => RegionKind::Other,
        };
        region
    }

    pub fn close_immediate(&mut self, messages: &mut MessageHandler) {
        let mut to_remove = Vec::new();

        for decl in self.decls.values() {
            if decl.decl.is_incomplete_type() {
                to_remove.push(decl.designator.clone());
                decl.error(
                    messages,
                    format!(
                        "Missing full type declaration of incomplete type '{}'",
                        &decl.designator
                    ),
                );
                decl.hint(messages, "The full type declaration shall occur immediately within the same declarative part");
            }
        }

        for designator in to_remove {
            self.decls.remove(&designator);
        }
    }

    pub fn close_extended(&mut self, messages: &mut MessageHandler) {
        let mut to_remove = Vec::new();

        for decl in self.decls.values() {
            if decl.decl.is_deferred_constant() {
                to_remove.push(decl.designator.clone());
                decl.error(messages, format!("Deferred constant '{}' lacks corresponding full constant declaration in package body", &decl.designator));
            } else if decl.decl.is_protected_type() {
                to_remove.push(decl.designator.clone());
                decl.error(
                    messages,
                    format!("Missing body for protected type '{}'", &decl.designator),
                );
            }
        }

        for designator in to_remove {
            self.decls.remove(&designator);
        }
    }

    pub fn close_both(&mut self, messages: &mut MessageHandler) {
        self.close_immediate(messages);
        self.close_extended(messages);
    }

    pub fn add(&mut self, decl: VisibleDeclaration<'a>, messages: &mut MessageHandler) {
        if self.kind != RegionKind::PackageDeclaration && decl.decl.is_deferred_constant() {
            decl.error(
                messages,
                "Deferred constants are only allowed in package declarations (not body)",
            );
        }

        match self.decls.entry(decl.designator.clone()) {
            Entry::Occupied(mut entry) => {
                let old_decl = entry.get_mut();

                if !decl.may_overload || !old_decl.may_overload {
                    if old_decl.is_deferred_of(&decl) {
                        if self.kind != RegionKind::PackageBody
                            && decl.decl.is_non_deferred_constant()
                        {
                            decl.error(messages, "Full declaration of deferred constant is only allowed in a package body");
                        }

                        std::mem::replace(old_decl, decl);
                    } else if let Some(ref pos) = decl.decl_pos {
                        let mut msg = Message::error(
                            pos,
                            format!("Duplicate declaration of '{}'", decl.designator),
                        );

                        if let Some(ref old_pos) = old_decl.decl_pos {
                            msg.add_related(old_pos, "Previously defined here");
                        }

                        messages.push(msg)
                    }
                }
            }
            Entry::Vacant(entry) => {
                if decl.decl.is_protected_type_body() {
                    decl.error(
                        messages,
                        format!("No declaration of protected type '{}'", &decl.designator),
                    );
                } else {
                    entry.insert(decl);
                }
            }
        }
    }

    pub fn make_library_visible(&mut self, library_name: &Symbol, library: &'a Library<'a>) {
        let name = VisibleDeclaration {
            designator: Designator::Identifier(library_name.clone()),
            decl_pos: None,
            decl: AnyDeclaration::Library(library),
            may_overload: false,
        };
        self.visible.insert(name.designator.clone(), name);
    }

    pub fn make_package_visible(&mut self, name: &Symbol, package: &'a PackageDesignUnit<'a>) {
        let name = VisibleDeclaration {
            designator: Designator::Identifier(name.clone()),
            decl_pos: None,
            decl: AnyDeclaration::Package(package),
            may_overload: false,
        };
        self.visible.insert(name.designator.clone(), name);
    }

    pub fn lookup(&self, designator: &Designator) -> Option<&VisibleDeclaration<'a>> {
        // @TODO do not expose declarations visible by use clause when used by selected name
        self.visible
            .get(designator)
            .or_else(|| self.decls.get(designator))
            .or_else(|| {
                self.parent
                    .as_ref()
                    .and_then(|parent| parent.lookup(designator))
            })
    }

    pub fn add_interface_list(
        &mut self,
        declarations: &'a [InterfaceDeclaration],
        messages: &mut MessageHandler,
    ) {
        for decl in declarations.iter() {
            for item in decl.declarative_items() {
                self.add(item, messages);
            }
        }
    }

    pub fn add_element_declarations(
        &mut self,
        declarations: &'a [ElementDeclaration],
        messages: &mut MessageHandler,
    ) {
        for decl in declarations.iter() {
            self.add(
                VisibleDeclaration::new(&decl.ident, AnyDeclaration::Element(decl)),
                messages,
            );
        }
    }
}

impl InterfaceDeclaration {
    fn declarative_items(&self) -> Vec<VisibleDeclaration> {
        match self {
            InterfaceDeclaration::File(InterfaceFileDeclaration { ref ident, .. }) => {
                vec![VisibleDeclaration::new(
                    ident,
                    AnyDeclaration::Interface(self),
                )]
            }
            InterfaceDeclaration::Object(InterfaceObjectDeclaration { ref ident, .. }) => {
                vec![VisibleDeclaration::new(
                    ident,
                    AnyDeclaration::Interface(self),
                )]
            }
            InterfaceDeclaration::Type(ref ident) => vec![VisibleDeclaration::new(
                ident,
                AnyDeclaration::Interface(self),
            )],
            InterfaceDeclaration::Subprogram(decl, ..) => vec![
                VisibleDeclaration::new(decl.designator(), AnyDeclaration::Interface(self))
                    .with_overload(true),
            ],
            InterfaceDeclaration::Package(ref package) => vec![VisibleDeclaration::new(
                &package.ident,
                AnyDeclaration::Interface(self),
            )],
        }
    }
}
