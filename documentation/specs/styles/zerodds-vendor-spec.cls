% ============================================================================
%  zerodds-vendor-spec.cls — ZeroDDS Vendor Specification document class.
%
%  Style follows the content-organisation conventions of OMG DDS Core
%  Specifications (DDS 1.4, DDSI-RTPS 2.5, DDS-Security 1.2, X-Types 1.3):
%
%    - numbered clauses (\S, \S\S, \S\S\S)
%    - normative-vs-informative annexes (Annex A normative, B informative,
%      C compliance test suite)
%    - explicit conformance-profile statements
%    - normative references as ISO-style table
%    - RFC 2119 keywords (MUST/SHOULD/MAY/SHALL) used consistently
%
%  Branding is ZeroDDS, not OMG. No OMG document number, no OMG title-page
%  layout. ZeroDDS publishes its own vendor specifications under its own
%  versioning scheme.
%
%  Build: `tectonic main.tex` (single binary, reproducible).
% ============================================================================

\NeedsTeXFormat{LaTeX2e}
\ProvidesClass{zerodds-vendor-spec}[2026/04/29 ZeroDDS Vendor Specification class]

\LoadClass[
  paper=a4paper,
  fontsize=11pt,
  parskip=half,
  numbers=noenddot,
  twoside=false,
  BCOR=0mm,
  DIV=12,
  toc=bibliography,
  toc=index
]{scrreprt}

\RequirePackage[utf8]{inputenc}
\RequirePackage[T1]{fontenc}
\RequirePackage[english]{babel}
\RequirePackage{lmodern}
\RequirePackage{microtype}
\RequirePackage{xcolor}
\RequirePackage{graphicx}
\RequirePackage{tabularx}
\RequirePackage{booktabs}
\RequirePackage{longtable}
\RequirePackage{enumitem}
\RequirePackage{listings}
\RequirePackage[most]{tcolorbox}
\RequirePackage{hyperref}
\RequirePackage{url}
\RequirePackage{afterpage}
\RequirePackage{tikz}
\usetikzlibrary{positioning,arrows.meta,shapes.geometric}

% ----------------------------------------------------------------------------
%  Brand colours.
% ----------------------------------------------------------------------------
\definecolor{zerodds-primary}{HTML}{1F3A5F}   % deep navy
\definecolor{zerodds-accent}{HTML}{0E7C66}    % teal
\definecolor{zerodds-light}{HTML}{F2F4F8}     % page-tint
\definecolor{zerodds-warn}{HTML}{B0541A}      % amber, used for warnings
\definecolor{zerodds-rule}{HTML}{C7CCD4}      % light grey rule

% ----------------------------------------------------------------------------
%  Hyperref colours.
% ----------------------------------------------------------------------------
\hypersetup{
  colorlinks=true,
  linkcolor=zerodds-primary,
  citecolor=zerodds-primary,
  urlcolor=zerodds-accent,
  pdfborder={0 0 0}
}

% ----------------------------------------------------------------------------
%  Spec front-matter macros.
% ----------------------------------------------------------------------------
\newcommand{\specshorttitle}[1]{\def\@specshorttitle{#1}}
\newcommand{\specversion}[1]{\def\@specversion{#1}}
\newcommand{\specstatus}[1]{\def\@specstatus{#1}}
\newcommand{\specdate}[1]{\def\@specdate{#1}}
\newcommand{\speceditors}[1]{\def\@speceditors{#1}}

\specshorttitle{ZeroDDS Vendor Specification}
\specversion{zerodds/v1.0-beta1}
\specstatus{Beta}
\specdate{\today}
\speceditors{Sandra Ke{\ss}ler (Editor-in-Chief)}

% ----------------------------------------------------------------------------
%  Title page.
% ----------------------------------------------------------------------------
\renewcommand{\maketitle}{%
  \begin{titlepage}
    \pagecolor{zerodds-light}
    \afterpage{\nopagecolor}
    \centering
    \vspace*{2.5cm}
    {\fontsize{32}{38}\selectfont\bfseries\color{zerodds-primary}
      ZeroDDS\par}
    \vspace{0.4cm}
    {\Large\color{zerodds-accent}\@specshorttitle\par}
    \vspace{2cm}
    {\fontsize{20}{24}\selectfont\bfseries\@title\par}
    \vspace{0.6cm}
    {\large\bfseries Version \textsf{\@specversion}\par}
    \vspace{0.3cm}
    {\large Status: \@specstatus\par}
    \vspace{0.2cm}
    {\normalsize Date: \@specdate\par}
    \vfill
    {\small Editors: \@speceditors\par}
    \vspace{0.4cm}
    {\footnotesize Copyright \textcopyright{} 2026 ZeroDDS Project. All rights reserved.\par}
    {\footnotesize This specification is published under the ZeroDDS Vendor Specification Licence.\par}
  \end{titlepage}
  \nopagecolor
}

% ----------------------------------------------------------------------------
%  Conformance profile box.
%
%  Usage:
%    \begin{conformance}{Endpoint Profile}
%      An implementation conforms to the Endpoint Profile if and only if ...
%    \end{conformance}
% ----------------------------------------------------------------------------
\newtcolorbox{conformance}[1]{
  enhanced,
  breakable,
  colback=zerodds-light,
  colframe=zerodds-primary,
  boxrule=0.6pt,
  arc=2pt,
  fonttitle=\bfseries\color{white},
  coltitle=white,
  colbacktitle=zerodds-primary,
  title={Conformance: #1},
  before skip=8pt,
  after skip=8pt,
  left=8pt, right=8pt, top=6pt, bottom=6pt
}

% ----------------------------------------------------------------------------
%  Note / Warning / Example admonitions.
% ----------------------------------------------------------------------------
\newtcolorbox{specnote}{
  enhanced, breakable,
  colback=zerodds-light, colframe=zerodds-rule,
  boxrule=0.4pt, arc=1pt,
  before skip=6pt, after skip=6pt,
  left=8pt, right=8pt, top=4pt, bottom=4pt,
  title={Note}, fonttitle=\bfseries\itshape
}

\newtcolorbox{specwarning}{
  enhanced, breakable,
  colback=zerodds-light, colframe=zerodds-warn,
  boxrule=0.6pt, arc=1pt,
  before skip=6pt, after skip=6pt,
  left=8pt, right=8pt, top=4pt, bottom=4pt,
  title={Warning}, fonttitle=\bfseries\itshape\color{zerodds-warn}
}

\newtcolorbox{specexample}[1][]{
  enhanced, breakable,
  colback=white, colframe=zerodds-accent,
  boxrule=0.4pt, arc=1pt,
  before skip=6pt, after skip=6pt,
  left=8pt, right=8pt, top=4pt, bottom=4pt,
  title={Example#1}, fonttitle=\bfseries\itshape\color{zerodds-accent},
  coltitle=zerodds-accent
}

% ----------------------------------------------------------------------------
%  Cross-spec reference macros.
%
%  \specref{dds-amqp-1.0}{\S 7.1}      — points at another section
%  \omgref{DDSI-RTPS 2.5}{\S 9.4}       — points at an OMG spec
% ----------------------------------------------------------------------------
\newcommand{\specref}[2]{\textit{#1}~#2}
\newcommand{\omgref}[2]{\textit{#1}~#2}

% RFC 2119 keyword macro — renders in small caps for visual emphasis.
\newcommand{\rfckw}[1]{\textsc{\MakeLowercase{#1}}}

% ----------------------------------------------------------------------------
%  IDL listing style.
% ----------------------------------------------------------------------------
\lstdefinelanguage{idl}{
  morekeywords={module,struct,union,switch,case,default,enum,typedef,
    sequence,string,wstring,interface,exception,raises,oneway,in,out,
    inout,context,attribute,readonly,const,native,valuetype,bitset,
    bitmask,bitfield,annotation,short,long,unsigned,float,double,
    boolean,char,wchar,octet,any,Object,fixed,TRUE,FALSE,FALSE},
  sensitive=true,
  morecomment=[l]{//},
  morecomment=[s]{/*}{*/},
  morestring=[b]"
}

\lstdefinestyle{idl-listing}{
  language=idl,
  basicstyle=\ttfamily\small,
  keywordstyle=\color{zerodds-primary}\bfseries,
  commentstyle=\color{zerodds-rule}\itshape,
  stringstyle=\color{zerodds-accent},
  numbers=none,
  frame=single,
  framesep=4pt,
  rulecolor=\color{zerodds-rule},
  backgroundcolor=\color{zerodds-light},
  showstringspaces=false,
  breaklines=true,
  captionpos=t,
  aboveskip=8pt,
  belowskip=8pt
}

\lstset{style=idl-listing}

% ----------------------------------------------------------------------------
%  Annex headings — Annex A (normative), B (informative), C (compliance).
%
%  Usage in main.tex:
%      \annex{A}{normative}{IDL Configuration Schema}
% ----------------------------------------------------------------------------
\newcommand{\annex}[3]{%
  \chapter*{Annex #1\\\normalsize\textit{(#2)}\\#3}
  \addcontentsline{toc}{chapter}{Annex #1 (#2): #3}
  \markboth{Annex #1: #3}{Annex #1: #3}
}

% ----------------------------------------------------------------------------
%  Section header style.
% ----------------------------------------------------------------------------
\addtokomafont{disposition}{\color{zerodds-primary}}
\addtokomafont{descriptionlabel}{\color{zerodds-primary}}

% Default header sizes (KOMA chapters are large; we tame them).
\setkomafont{chapter}{\Huge\bfseries\color{zerodds-primary}}
\setkomafont{section}{\Large\bfseries\color{zerodds-primary}}
\setkomafont{subsection}{\large\bfseries\color{zerodds-primary}}

% ----------------------------------------------------------------------------
%  TOC layout — KOMA defaults reserve too little space for deep numbering
%  (3.X.Y.Z). We widen the indent so long titles don't wrap into the
%  numbering column.
% ----------------------------------------------------------------------------
\RedeclareSectionCommand[tocnumwidth=3.0em]{section}
\RedeclareSectionCommand[tocnumwidth=3.6em]{subsection}
\RedeclareSectionCommand[tocnumwidth=4.4em]{subsubsection}

% ----------------------------------------------------------------------------
%  Page-style footer with Spec-Title + Version.
% ----------------------------------------------------------------------------
\RequirePackage{scrlayer-scrpage}
\pagestyle{scrheadings}
\clearpairofpagestyles
\ohead{\small\@specshorttitle, \textsf{\bfseries\@specversion}}
\ofoot{\small\thepage}
\KOMAoptions{footsepline=true}

\endinput
