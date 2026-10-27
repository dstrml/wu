(XMLDecl "xml" @keyword)

[ "version" "encoding" "standalone" ] @attribute

[ (VersionNum) (EncName) ] @string

[ "yes" "no" ] @boolean

(PI (PITarget) @keyword)

(XmlModelPI "xml-model" @keyword)

(StyleSheetPI "xml-stylesheet" @keyword)

(PseudoAtt (Name) @attribute)

(PseudoAtt (PseudoAttValue) @string)

(doctypedecl "DOCTYPE" @tag.doctype)

(doctypedecl (Name) @type)

[ "PUBLIC" "SYSTEM" ] @keyword

(PubidLiteral) @string

(SystemLiteral) @string

(elementdecl
  "ELEMENT" @keyword
  (Name) @tag)

(contentspec
  (_ (Name) @property))

"#PCDATA" @type.builtin

[ "EMPTY" "ANY" ] @constant.builtin

(GEDecl
  "ENTITY" @keyword
  (Name) @constant)

(GEDecl (EntityValue) @string)

(NDataDecl
  "NDATA" @keyword
  (Name) @label)

(PEDecl
  "ENTITY" @keyword
  "%" @operator
  (Name) @constant)

(PEDecl (EntityValue) @string)

(NotationDecl
  "NOTATION" @keyword
  (Name) @constant)

(AttlistDecl
  "ATTLIST" @keyword
  (Name) @tag)

(AttDef (Name) @attribute)

(AttDef (Enumeration (Nmtoken) @string))

(DefaultDecl (AttValue) @string)

[
  (StringType)
  (TokenizedType)
] @type.builtin

(NotationType "NOTATION" @type.builtin)

[
  "#REQUIRED"
  "#IMPLIED"
  "#FIXED"
] @keyword

[ (EntityRef) (CharRef) (PEReference) ] @string.escape

(STag (Name) @tag)

(ETag (Name) @tag)

(EmptyElemTag (Name) @tag)

(Attribute (Name) @attribute)

(Attribute (AttValue) @string)

[
  "<?" "?>"
  "<!" "]]>"
  "<" ">"
  "</" "/>"
] @punctuation.bracket

[ "(" ")" "[" "]" ] @punctuation.bracket

[ "*" "?" "+" "," "|" "=" ] @operator

(CDSect (CDStart) @punctuation.bracket)

(CDSect (CData) @text.literal)

(Comment) @comment
