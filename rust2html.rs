use syntax::codemap;
use syntax::ast;
use syntax::codemap::Pos;
use rustc::middle::ty;
use iou=ioutil;
use std::hash::Hash;
use collections::HashMap;
use std::slice;
use std::cmp;
use codemaput::{ZIndexFilePos,ToZIndexFilePos};
use find_ast_node::{FNodeInfoMap,FNodeInfo};
use rfindctx::{RFindCtx};
use crosscratemap::{CrossCrateMap};
use jumptodefmap::{JumpToDefMap};
use rsfind::MyOption;
//use self::htmlwriter::HtmlWriter;
use timer::Profiler;

mod htmlwriter;


// TODO: See how far we can decouple this from the link generation
// could we have an abstract linked-AST format , and an html renderer for that..
// eg incase this can be spun off as a completely seperate project
// eg: could this be done more efficiently client-side-throw json +javascript decoder at the browser.
// is there an existing cross-language linked-source generator? if not, could one be written? (would it get more traction?)

// TODO: view_item 'mod path::....' could be converted to file reference?

// TODO - how to handle out-of-crate references?
// perhaps we could detect items with def_id crate indices outside the current crate,
// and link to their *nodes* instead of source-lines? or would the spans still work?
// we could give the whole generator a root dir to look for crates? ... and assume html is generated in there..

// todo: options struct.
pub struct Options {
    pub write_file_path: bool,
    pub write_references: bool,
    pub output_dir: Path
}

impl Options {
    pub fn default() -> Options {
        Options {
            write_file_path: true,
            write_references: true,
            output_dir: Path::new("")
        }
    }
}


//nim:&FNodeInfoMap,jdm:&JumpToDefMap, jrm:&JumpToRefMap
pub fn make_html(dc: &RFindCtx, fm: &codemap::FileMap, nmaps: &NodeMaps,
                 xcm: &CrossCrateMap, fln: &FileLineNodes, lib_path: &str, 
                 out_file: &Path, options: &Options) -> ~str {
    // todo - Rust2HtmlCtx { fm,nim,jdm,jrm } .. cleanup common intermediates
	let mut p=Profiler::new("make_html");

    let mut doc= htmlwriter::HtmlWriter::new();
    write_head(&mut doc, out_file, options);

    let hash=get_str_hash(fm.name);
//  let bg=(~[~"383838",~"34383c",~"3c3834",~"383c34",~"343c38",~"38343c",~"3a343a",
//          ~"3a343a",~"36363a",~"363a36",~"3a3636",~"3a3a34",~"3a333a",~"343a3a",~"343a3c",~"343838"])[hash&15];
    // write the doc lines..
    let body=doc.begin_tag_check("body");//,&[(~"style",~"background-color:#"+bg+";")]);
    let div=doc.begin_tag_check("div");//,&[(~"style",~"background-color:#"+bg+";")]);
//    let bg_tag=doc.begin_tag_check("bg"+(hash&15).to_str()); - todo -we had tinted different pages, doesn't seem to work well
    let maintext=doc.begin_tag_check("maintext");
    let fstart = fm.start_pos;
    let max_digits=num_digits(fm.lines.borrow().len());

    if options.write_file_path {
        let t0=doc.begin_tag_check("div");//,&[(~"style",~"background-color:#"+bg+";")]);
        let t1=doc.begin_tag_check("fileblock");
        doc.write_path_links(fm.name);
        doc.end_tag_check(t1);
        doc.end_tag_check(t0);
    }
    {
        let mut scw=SourceCodeWriter::new(&mut doc);
        for line in range(0, fm.lines.borrow().len()) {
            scw.line_index=line;
            // todo: line numbers want to go in a seperate column so they're unselectable..
            scw.doc.write_tagged("ln",pad_to_length((line+1).to_str(),max_digits," "));
            scw.doc.begin_tag_anchor((line+1).to_str()); 
            let lend = if line < (fm.lines.borrow().len()-1) {
                (fm.lines.borrow().get(line+1) - fstart).to_uint()
            } else {
                fm.src.len()
            };
            scw.doc.write(" ");
            let line_str = fm.src.slice((fm.lines.borrow().get(line) - fstart).to_uint(), lend);
            //doc.writeln(line_str);
			
            scw.doc.end_tag();
            
            // this block appears to be what corrupted it - is it anything to do with the iteration?
            
            let depth=scw.doc.depth();
            let null_line=~[];
            let def_nodes_on_line=fln.def_nodes_per_line.get(line).unwrap_or(&null_line); // caution, because what it returned was Option, it seems.
            for nl in def_nodes_on_line.iter() {
                scw.doc.begin_tag_anchor("n"+nl.to_str());
            }
            scw.doc.write(" "); // TODO, It should be ok to nest these surely.
            for _ in def_nodes_on_line.iter() {
                scw.doc.end_tag();
            }
            scw.doc.check_depth(depth);
            
            
            write_line_with_links(&mut scw,dc,fm,lib_path, nmaps,xcm, line_str, fln.nodes_per_line[line]);
            scw.doc.writeln("");
    //      doc.writeln(markup_line);
        }
    }
    if options.write_references {
        write_references(&mut doc,dc,fm,lib_path,nmaps, fln.nodes_per_line);
    }

    doc.end_tag_check(maintext);
//    doc.end_tag_check(bg_tag);
    doc.end_tag_check(div);
    doc.end_tag_check(body);

    doc.doc
}

pub fn write_source_as_html_sub(dc:&RFindCtx, nim:&FNodeInfoMap, jdm:&JumpToDefMap,xcm:&CrossCrateMap,lib_path:&str, options: &Options) {

    let npl=NodesPerLinePerFile::new(dc,nim);

//  let nspl=~[~[]];
    let mut def2refs = ~MultiMap::new();
    for (&nref,&ndef) in jdm.iter() {
        if ndef.krate==0 {
            def2refs.insert(ndef.node,nref);
        }
    };

    let nmaps=NodeMaps { nim:nim, jdm:jdm, jrm:def2refs};
    let files=&dc.codemap().files;
    let files = files.borrow();
    for (fi,fm) in files.iter().enumerate() {
        if is_valid_filename(fm.name) {
            let html_name = options.output_dir.join(Path::new(make_html_name(fm.name)));
            println!("generating {}: {}..", fi.to_str(), html_name.display());
            let doc_str=make_html(dc, &**fm, &nmaps,xcm, &npl.file[fi] , lib_path,
                                  &html_name, options);
            iou::fileSaveStr(doc_str, &html_name);
        }
    }

    // copy all resources to the output folder

// can't be bothered with making this data driven now.
// just have some styles compiled in, and maybe have a switch light/dark theme to begin with.
// its more important that it actually works without faffing around with paths. Its just some colored text
//
//	match iou::copy_folder(&Path::new("resources/"), &options.output_dir) {
//		Ok(_)	=>{},
//		Err(_)	=>{
//			println!("Could not copy resources, so copying default CSS\n");

	{
		use std::io;
		use std::io::fs;
		fs::mkdir(&Path::new("css"),io::UserDir);
		match io::fs::File::create(&Path::new("css/sourcestyle.css")) {
			Err(_) => println!("can't write css/sourcestyle.css"),
			Ok(mut file)=>{
				println!("Writing default CSS");
				file.write(g_default_css);
			}
		}
	}
}

static g_default_css:&'static [u8]=include_bin!("resources/css/sourcestyle.css");

fn is_valid_filename(f:&str) ->bool{
    match f.chars().nth(0) {
        Some(c)=> match c{
            '<' => false,
            _=>true
        },
        None => false
    }
}

fn write_head(doc:&mut htmlwriter::HtmlWriter, out_file: &Path, options: &Options) {
    let css_rel_path = &options.output_dir
                        .path_relative_from(&out_file.dir_path())
                        .unwrap_or(Path::new(""));
    fn write_css_link (doc: &mut htmlwriter::HtmlWriter, css_rel_path: &Path, stylesheet_name: &str) {
        let path = match css_rel_path.with_filename(stylesheet_name).as_str() {
            Some(s) => s,
            None => stylesheet_name
        }.to_owned();
        doc.write_tag_ext("link", &[(~"href", path), (~"rel", ~"stylesheet"), (~"type", ~"text/css")]);
    };

    doc.begin_tag("head");
    write_css_link(doc, css_rel_path, "css/shCore.css");
    write_css_link(doc, css_rel_path, "css/shThemeDefault.css");
    write_css_link(doc, css_rel_path, "css/sourcestyle.css");
    doc.end_tag();
}

fn get_str_hash(s:&str)->uint{
    let mut acc=0;
    for x in s.chars() { acc=((acc<<5)-acc)^(acc>>12); acc+=x as uint;}
    acc
}



fn num_digits(a:uint)->uint{
    let mut n=1;
    let mut aa=a;
    while aa>=10 { aa/=10; n+=1;}
    n
}
fn pad_to_length(a:&str,l:uint,pad:&str)->~str {
    let mut acc=~" ";
    let mut i=(l-a.len()) as int;
    while i>0 {
        acc.push_str(pad);
        i-=1;//pad.len() as int;
    }
    acc.push_str(a);
    acc
}

pub struct FileLineNodes {
    nodes_per_line:~[~[ast::NodeId]],
    def_nodes_per_line:~[~[ast::NodeId]]
}
struct NodesPerLinePerFile {
    file :~[FileLineNodes]
}
//type NodesPerLine=&[~[ast::NodeId]];

pub fn get_file_index(dc:&RFindCtx,fname:&str)->Option<uint> {
    // todo - functional
    let mut index=0;
    let files = dc.codemap().files.borrow();
    while index<files.len() {
        if fname == files.get(index).name {
            return Some(index);
        }
        index+=1;
    }
    None
}
pub fn get_crate_name(dc:&RFindCtx,ci:ast::CrateNum)->~str {
    super::codemaput::get_crate_name(dc.tycx_ref(),ci)
}


impl NodesPerLinePerFile {
    fn new(dc:&RFindCtx, nim:&FNodeInfoMap)->~NodesPerLinePerFile {
        // todo, figure this out functionally?!
        //      dc.sess.codemap.files.map(
        //              |fm:&@codemap::FileMap|{ slice::from_elem(fm.lines.len(), ~[]) }
        //          ).collect();

        let mut npl=~NodesPerLinePerFile{file:~[]};
//      let mut fi=0;
//      npl.file = slice::from_elem(dc.sess.codemap.files.len(),);
        let files = dc.codemap().files.borrow();
        for cmfile in files.iter() {
//      while fi<dc.sess.codemap.files.len() {
//          let num_lines=dc.sess.codemap.files[fi].lines.len();
            let num_lines=cmfile.lines.borrow();
            let num_lines = num_lines.len();
            npl.file.push(FileLineNodes{
                nodes_per_line: slice::from_elem(num_lines,~[]),
                def_nodes_per_line: slice::from_elem(num_lines,~[])
            });
//          fi+=1;
        };
        for (k,v) in nim.iter() {
            // TODO- only want the **DEF_NODES** for 'def_nodes_per_line', not all.
            // todo, this could be more direct, file index, line index, ...
            v.span.lo.to_index_file_pos(dc.tycx_ref()).for_some(|ifp|{
                match v.span.hi.to_index_file_pos(dc.tycx_ref()) {
                    None=>{ },
                    Some(ifpe)=>{

                        let f = &mut npl.file [ifp.file_index as uint];
                        f.def_nodes_per_line [ifp.line as uint].push(*k);
//                      dump!(ifp, f.nodes_per_line.len());
                        for li in range(ifp.line,ifpe.line+1) {
                            if li < f.nodes_per_line.len() as u32 {
//                              dump!(li, *k)
                                f.nodes_per_line[li as uint].push(*k)
                            };
                        };
                    }
                };
            });
        };
//      npl.dump();
        npl
    }
    #[allow(dead_code)]
    fn dump(&self) {
        for f in self.file.iter() {
            print!("file \\{");
            for l in f.nodes_per_line.iter() {
                print!("line \\{{}", l.len().to_str());
                for n in l.iter() {
                    print!("{},", n.to_str());
                }
                print!("\\} line");
            }
            print!("\\}file");
        }
    }
}
pub fn node_color_index(ni:&FNodeInfo)->int {
    // TODO - map this a bit more intelligently..
    match ni.kind.as_slice() {
        "fn"=>1,
        "add"|"sub"|"mul"|"div"|"assign"|"eq"|"le"|"gt"|"ge"|"ne"|"binop"|"assign_op"
        |"bitand"|"bitxor"|"bitor"|"shl"|"shr"|"not"|"neg"|"box"|"uniq"|"deref"|"addr_of"
            =>5,
        "de"=>3,
        "type_param"=>7,
        "ty"=>8,
        "struct_field"|"field"=>24,
        "path"=>26,
        "call"=>27,
        "variant"=>28,
        "method_call"=>10,
        "lit"=>12,
        "stmt"=>13,
        "mod"=>38,
        "local"=>16,
        "pat"=>20,
        "block"|"blk"|"fn_block"=>22,
        "method"|"type_method"=>18,
        "tup"=>14,
        "arm"=>11,
        "index"=>13,
        "vstore"=>16,
        "mac"=>10,
        "struct"=>31,
        "trait"=>32,
        "impl"=>33,
        "enum"=>34,
        "keyword"|"while"|"match"|"loop"|"do"|"cast"|"if"|"return"|"unsafe"|"extern"|"crate"|"as"|"in"|"for"=>21,

        _ =>1
    }
}
pub fn color_index_to_tag(i:int)->~str {
    "c"+i.to_str()
}
fn is_alphanumeric(c:char)->bool {
    match c {
        'a'..'z'|'A'..'Z'|'0'..'9'|'_'=>true,
        _=>false
    }
}
fn is_alphanumeric_u8(c:u8)->bool {
    match c as char{
        'a' ..'z'|'A'..'Z'|'0'..'9'|'_'=>true,
        _=>false
    }
}
type ColorIndex=int;
/// todo .. option type!!!
fn is_text_here(line:&str, pos:uint,reftext:&str, color:ColorIndex)->Option<(ColorIndex,uint)> {
    if pos+reftext.len()+2>=line.len() { return None}
    if pos>0 {
        if is_alphanumeric(line[pos-1] as char) { return None }// must be word start
    }
    if is_alphanumeric(line[pos+reftext.len()] as char) { return None }// must be word start
    let mut i=0;
    while i<reftext.len() {
        if reftext[i]!=line[i+pos] {return None}
        i+=1;
    }

    return Some((color,reftext.len()));
}

fn sub_match(line:&str,x:uint,opts:&[&str])->Option<(uint,uint)>{
    let smp=x;
    if is_alphanumeric_u8(line[smp])==false{ return None;} // not alpha
    let mut opti=0;
    if line.is_char_boundary(x)==false { return None; }
    for o in opts.iter() {
        if (o.len()+x)>line.len() { continue; } // not enough chars in line
        if (o.len()+x+1)<line.len() {
            if is_alphanumeric_u8(line[o.len()+x]) { continue; }// not word boundary for this one
        }

        let ex=o.len()+x;
        if line.is_char_boundary(ex) {
            if line.slice(x,ex) == o.slice(0,o.len()) {
                return Some((opti,o.len()));
            }
        }
        opti+=1;

    }
    return None;
}

pub struct NodeMaps<'a>  {
    nim:&'a FNodeInfoMap,
    jdm:&'a JumpToDefMap,
    jrm:&'a JumpToRefMap
}

/// interface for emitting source code serially
/// carries state between emitting lines
struct SourceCodeWriter<'a, T> {
    doc: &'a mut T,
    multiline_comment_depth:int,
    brace_depth:int,
    bracket_depth:int,
    angle_bracket_depth:int,
    in_string:int,
    line_index:uint
}

impl<'a, T> SourceCodeWriter<'a,T> {
    fn new(d:&'a mut T)->SourceCodeWriter<'a, T> {
        SourceCodeWriter{
            doc:d, multiline_comment_depth:0, brace_depth:0,bracket_depth:0, angle_bracket_depth:0, in_string:0, line_index:0
        }
    }
}

static no_link:i64  =0 ;
static link_to_refs:bool    =true;
static link_debug:bool      =true;


fn write_line_with_links(dst:&mut SourceCodeWriter<htmlwriter::HtmlWriter>,dc:&RFindCtx,fm:&codemap::FileMap,lib_path:&str, nmaps:&NodeMaps,xcm:&CrossCrateMap, line:&str, nodes:&[ast::NodeId]) {
    // todo ... BREAK THIS FUNCTION UP!!!!
    // and there is a load of messy cut paste too.

//  for x in node_infos.iter() { println!(fmt!("%?", x));}
//  dump!(node_infos);
// todo - sorting node spans, not this "painters-algorithm" approach..

    let mut link:~[i64] = slice::from_elem(line.len(),0 as ast::NodeId as i64);
    let mut color:~[int] = slice::from_elem(line.len(),0 as int);
    let mut depth:~[uint]= slice::from_elem(line.len(),0x7fffffff as uint);

    for n in nodes.iter() {

        match nmaps.nim.find(n) {
            None=>{},
            Some(ni)=>{
                // link_id >0 = node def link. link_id<0 = node ref link

                let link_id= match nmaps.jdm.find(n) {
                    None=>
                        if link_to_refs{
                            if nmaps.jrm.find(*n  ).len()>0 {
                                - *n as i64
                            } else {
                                no_link
                            }
                        }else{
                            no_link
                        },
                    Some(x) => if link_debug{(x.node as i64)|(x.krate as i64<<48)} else {x.node as i64}
                };

//              let null_def=ast::def_id{crate_:0,node:0};
//              let x=nmaps.jdm.find(n).unwrap_or_default(&null_def);
//              let link_id=(x.node as i64)|(x.crate as i64<<48);
//              let link_id=*n as i64 &15;

                let os=ni.span.lo.to_index_file_pos(dc.tycx_ref());
                let oe=ni.span.hi.to_index_file_pos(dc.tycx_ref());
                if os.is_some() && oe.is_some() {
                    let e=oe.unwrap(); let s=os.unwrap();
                    let d = ni.span.hi.to_uint() - ni.span.lo.to_uint();    // todo - get the actual hrc node depth in here!
                    let xs = if dst.line_index <= s.line as uint {
                        s.col
                    } else {
                        0
                    };
                    // TODO: instead of this brute force 'painters-algorithm',
                    // clip spans, or at least have a seperate "buffer" for the line-coherent infill
                    // for multi-line spans

                    // 'paint' the nodes we have here.
                    //if (s.line==e.line)
                    let xe = if e.line as uint > dst.line_index {
                        line.len()
                    }else{
                        e.col as uint
                    };
                    let ci = node_color_index(ni);
                    for x in range(xs as uint, cmp::min(xe, line.len())) {
                        if d <= depth[x] {
                            color[x]=ci;
                            depth[x] = d;
                            if link_id!=0 {
                                link[x]=link_id;
                            }
                        }
                        if link[x]==0 { link[x]=link_id;}
                    }
                }
            }
        }
    }


    // paint comments out, mark delimiter symbols--override what we get from buggy tree picture...
    // TODO ... need to figure out tree nodes encompasing the current line from above to
    // propogate information properly eg brackets inside a type ..

    {
        // handle decls, todo - combine with keywords..
        let mut x;
        let mut wb;
        //attributes/"preprocessor"
        if line[0]as char =='#' {
            for x in range(0,line.len()) { color[x]=50 }
        }
        // delimiters/whitespace/special chars characters - disable links & override some coloring
        x=0;
        while x < line.len() {
            let c0=line[x] as char;

            match c0 {
//              ' '|'\t'|'+'|'-'|'|'|':'|'*'|'&'|'\''|'/'|'@'|'~'|'^'|'%'|'$'|'!'|'>'|'<'|'.'|'#'=> {link[x]=0;}
                '{'|'}'|'['|']'|';'|',' => {color[x]=3; link[x]=0; },
                '('|')'=> {color[x]=4;link[x]=0;},
                _=>{}
            }
            if x<(line.len()-1) {
                let c1=line[x+1] as char;
                if (c0=='-' || c0=='=') && c1=='>' {
                    color[x]=4;color[x+1]=4; link[x]=0; link[x+1]=0;
                    x+=1;
                }
            }

            x+=1;
        }
        x=0; wb=true;
        while x<(line.len()) {
            // override color for top level decls
            if wb && is_alphanumeric(line[x] as char){
                let (decl_color,len)=
                                        is_text_here(line,x,"fn",30)
                    .unwrap_or( is_text_here(line,x,"struct",31)
                    .unwrap_or( is_text_here(line,x,"trait",32)
                    .unwrap_or( is_text_here(line,x,"impl",33)
                    .unwrap_or( is_text_here(line,x,"enum",34)
                    .unwrap_or( is_text_here(line,x,"type",35)
                    .unwrap_or( is_text_here(line,x,"static",36)
                    .unwrap_or( is_text_here(line,x,"macro_rules!",37)
                    .unwrap_or( is_text_here(line,x,"mod",38)
                    .unwrap_or( is_text_here(line,x,"class",39)
                    .unwrap_or((0,0)))))))))));
                if decl_color>0{
                    for x in range(x,x+len) { link[x]=0;/* clear link on the keyword part..*/}
                    let mut in_typaram=0;
                    while (x<line.len()) && (line[x] as char)!='{' && (line[x] as char)!='('{
                        in_typaram+=match line[x] as char {'<'=>1,_=>0};
                        color[x]=if in_typaram==0{decl_color}else{5};
                        in_typaram+=match line[x] as char {'>'=>-1,_=>0};
                        x+=1;
                    }
                }
            } else {
                wb= !is_alphanumeric_u8(line[x]);
            }
            x+=1;
        }
                // paint keywords out, these dont seem to come through as distinct ast nodes - as they can be differnt parts of a node
        x=0; wb=true;
        while x<line.len() {
            if wb {
                match sub_match(line,x,&[&"let",&"mut", &"const", &"use", &"mod", &"match",&"if",&"else",&"break",&"return",&"while",&"loop",&"for",&"do",&"ref",&"pub",&"priv",&"unsafe",&"extern",&"in",&"as",&"crate"]) {
                None=>{},
                Some((_, len))=>{
                    let me=x+len;
                    while x<me { color[x]=21; link[x]=0;x+=1; }
                    continue;
                    }
                }
            }
            if is_alphanumeric_u8(line[x]) {wb=false}
            x+=1;
        }

        // paint comments out, & disable links
        // TODO - multiline comments, feed in/out..
        x=0;
        while x<(line.len()) {
            if x<line.len()-1 {
                if line[x+0]=='/' as u8 && (line[x+1]=='/' as u8|| line[x+1]=='*' as u8) || dst.multiline_comment_depth>0{
                    let mut comment_color=40;
                    let mut slc=false;
                    if line[x]=='/' as u8 && line[x+1]=='*' as u8 {dst.multiline_comment_depth+=1;} else {slc=true;}
                    if dst.multiline_comment_depth>0 {comment_color=41;}
                    if x<line.len()-2 { if line[x+2]as char=='/' { comment_color=42 }}// doc-comments are a bit brightest
                    while x<line.len() && (slc || dst.multiline_comment_depth>0){
                        color[x]=comment_color;
                        link[x]=0;

                        x+=1;
                        if line[if x>=2{x-2}else{0}]=='*' as u8 && line[x-1]=='/' as u8 {dst.multiline_comment_depth-=1;}
                    }
                }
            }
            x+=1;
        }
    }
    let resolver=|x|resolve_link(x,dc,fm,lib_path, nmaps,xcm);
    write_line_attr_links(dst,line,color,link, resolver );
}

fn resolve_link(link:i64, dc:&RFindCtx,fm:&codemap::FileMap,lib_path:&str, nmaps:&NodeMaps,xcm:&CrossCrateMap)->~str {
/*
    if link_debug==false {
        if link!=no_link {
            if link>0 // value is link node index {
                match nmaps.nim.find(&link) {
                    None=>~"",  // link outside the crate?
                    Some(link_node_info)=>{
                        let oifp = link_node_info.span.lo.to_index_file_pos(dc.tycx);
                        match oifp {
                            Some(ifp)=>{
                                let link_str:~str="#"+(ifp.line+1).to_str();
                                make_html_name_rel(dc.sess.codemap.files[ifp.file_index].name,fm.name)+link_str
                            },
                            None=>{
                                // out of crate def node?
//                                  def_node = ;
                                ~"#n"+link.to_str()
                            }
                        }
                    }
                }
            } else if link<0{// link to refs block,value is -(this node index)
                let ifp= (nmaps.nim,-link).to_index_file_pos(dc.tycx).unwrap();

                "#"+(ifp.line+1).to_str()+"_"+ifp.col.to_str()+"_refs"
            }
        } else {
            ~""
        }
    } else
    */
    if link !=no_link {
        if (link as i32)<0{// link to refs block,value is -(this node index)
            let ifp= (nmaps.nim,-((link as i32) as u32)).to_index_file_pos(dc.tycx_ref()).unwrap();
            "#"+(ifp.line+1).to_str()+"_"+ifp.col.to_str()+"_refs"
        } else
        {
            let def_crate = (link>>48) as u32;
            let def_node=(link&((1<<48)-1)) as u32;
            match xcm.find(&ast::DefId{krate:def_crate,node:def_node}) {
                None=>//"#n"+def_node.to_str(), by node linnk
                {
                    match (nmaps.nim,def_node).to_index_file_pos(dc.tycx_ref()) {
                        Some(ifp)=>{
                            let files = dc.codemap().files.borrow();
                            make_html_name_rel(files.get(ifp.file_index as uint).name, fm.name) +
                                "#" + (ifp.line + 1).to_str()
                        },
                        None=>~"c="+def_crate.to_str()+" n="+def_node.to_str()
                    }

                },
                Some(a)=>{
    //                          "../gplsrc/rust/src/"+a.fname+".html"+
                    make_html_name_reloc(a.fname,fm.name,lib_path)+
                        "#n"+def_node.to_str()
                }
            }
        }
    } else {
        ~"no link"
    }

}


fn write_line_attr_links(dst:&mut SourceCodeWriter<htmlwriter::HtmlWriter>,text_line:&str,color:&[int],links:&[i64], resolve_link: |i64| -> ~str) {
    // emit a span..
    let no_color=-1;
    let mut curr_col=no_color;
    let mut curr_link=no_link;
    //let mut outp=HtmlWriter::new();
    let tag_depth=dst.doc.depth();
    assert!(text_line.len()==color.len() && text_line.len()==links.len());

    for x in range(0,text_line.len()) {
        // if state changed...
        if (curr_link,curr_col)!=(links[x],color[x]) {
            if curr_link !=no_link {dst.doc.end_tag();}
            if curr_col !=no_color {dst.doc.end_tag();}

            curr_col = color[x];
            curr_link=links[x];
            if curr_col !=no_color {
                dst.doc.begin_tag(color_index_to_tag(curr_col));
            }
            if curr_link !=no_link {
                dst.doc.begin_tag_link( resolve_link(links[x]) );
            }
        }
        dst.doc.write_u8_(text_line[x]);
    }
    if curr_col !=no_color {dst.doc.end_tag();}
    if curr_link !=no_link {dst.doc.end_tag();}
    assert!(tag_depth==dst.doc.depth());
}

fn find_defs_in_file(fm:&codemap::FileMap, nim:&FNodeInfoMap)->~[ast::NodeId] {
    // todo - functional way..
    let mut acc=~[];
    for (n,info) in nim.iter() {
        if info.span.lo >= fm.start_pos && (info.span.lo < (fm.start_pos+codemap::BytePos(fm.src.len() as u32))) {
            acc.push(*n);
        }
    }
    acc
}

/// K:[V]  insert(K, V) for many V;  find(K)->[V]
pub struct MultiMap<K,V> {
    next_index:uint,
    indices:HashMap<K,uint>,
    items:~[~[V]],
    empty:~[V]
}
impl<'a,K:Hash+TotalEq,V> MultiMap<K,V> {
    pub fn new()->MultiMap<K,V> {
        MultiMap{ next_index:0, indices:HashMap::new(), items:~[], empty:~[] }
    }
    pub fn find(&'a self, k:K)->&'a~[V] {
        // TODO - return iterator, not collection
        match self.indices.find(&k) {
            None=>&self.empty,
            Some(&ix)=>&self.items[ix]
        }
    }
    pub fn insert(&'a mut self, k:K,v:V) {
        let ix=match self.indices.find(&k) {
            None=>{ self.indices.insert(k,self.next_index); self.next_index+=1; self.items.push(~[]); self.next_index-1},
            Some(&ix)=> ix
        };
        self.items[ix].push(v);
    }
}

pub type JumpToRefMap = MultiMap<ast::NodeId, ast::NodeId>;

// TODO: find nodes of enclosing context.
// 'this function, called from these locations'
// 'this function, called fromm these functions ... <<< BETTER
// 'this type, used in these functions ... '

fn get_source_line(fm:&codemap::FileMap, i: uint) -> ~str {

    let lines = fm.lines.borrow();
    let le = if (i as uint) < (lines.len() - 1) {
        lines.get(i as uint + 1).to_uint()
    } else {
        (fm.src.len() as uint) + fm.start_pos.to_uint()
    };
//  dump!(fm.lines[i-1],*fm.start_pos, fm.lines[i-1]-le);
    if i > 0 {
        fm.src.slice((lines.get(i as uint) - fm.start_pos).to_uint(), (le - fm.start_pos.to_uint()) as uint).to_owned()
    } else {
        ~""
    }
}
fn num_source_lines(fm:&codemap::FileMap)->uint {
    let lines = fm.lines.borrow();
	lines.len()
}

//fn split_by_key<T,K>(src:&[T],f:&fn(t:&T)->K)->(K,[&T])] {
//}
#[deriving(Clone)]
pub struct Extents<T> {
    lo:T, hi:T
}

macro_rules! min(
    ($left:expr, $right:expr) => (
		{	let a=$left.clone(); let b=$right.clone(); // evaluate once
        	if a < b {
	            a.clone()
	        } else  {
	            b.clone()
	        }
        }
	)
)
macro_rules! max(
    ($left:expr, $right:expr) => (
        {	let a=$left.clone(); let b=$right.clone(); // evaluate once
        	if a > b {
	            a.clone()
	        } else  {
	            b.clone()
	        }
        }
    )
)

impl<T:Ord+Clone> Extents<T> {
    pub fn new(lo:&T,hi:&T)->Extents<T> { Extents{lo:lo.clone(),hi:hi.clone()} }
    pub fn new_from_value(v:&T)->Extents<T>{ Extents {lo:v.clone(),hi:v.clone()} }
    pub fn contains(&self, other:&Extents<T>)->bool {
        other.lo >= self.lo && other.hi <=self.hi
    }
    pub fn intersection(&self,other:&Extents<T>)->Option<Extents<T>> {
        if self.overlaps(other) {
            Some(Extents{lo: min!(self.lo, other.lo), hi: max!(self.hi, other.hi)})
        } else {
            None
        }
    }
    pub fn include(&self,other:&Extents<T>)->Extents<T> {
        Extents{lo: min!(self.lo, other.lo), hi: max!(self.hi, other.hi)}
    }
    pub fn overlaps(&self, other:&Extents<T>)->bool {
        !(other.lo >= self.hi || other.hi <= self.lo)
    }
    pub fn contains_val(&self, value:&T)->bool {
        *value >= self.lo && *value <= self.hi
    }
    pub fn include_val(&self, value:&T)->Extents<T> {
        Extents { lo: min!(self.lo, *value), hi: max!(self.hi, *value) }
    }
}



fn write_references(doc:&mut htmlwriter::HtmlWriter,dc:&RFindCtx, fm:&codemap::FileMap, _:&str,  nmaps:&NodeMaps, _: &[~[ast::NodeId]]) {


    let depth=doc.depth();
    doc.begin_tag_ext("div",[(~"class",~"refblock")]);

//  let (nim,jdm,jrm)=(nmaps.nim, nmaps.jdm, nmaps.jrm);
    let file_def_nodes = find_defs_in_file(fm,nmaps.nim);
    //let mut defs_to_refs=MultiMap::new::<ast::NodeId, ast::NodeId>();


    for &dn in file_def_nodes.iter() {
        let opt_def_info = nmaps.nim.find(&dn);
        if !opt_def_info.is_some() {continue;}
        let def_info = opt_def_info.unwrap();
        if !(def_info.kind==~"fn" || def_info.kind==~"struct" || def_info.kind==~"trait" || def_info.kind==~"enum" || def_info.kind==~"ty") { continue; }

        let refs = nmaps.jrm.find(dn);
        let max_links=30;   // todo - sort..
        let max_short_links=60; // todo - sort..


        if refs.len()>0 {
            let mut header_written=false;
            let opt_def_tfp = def_info.span.lo.to_index_file_pos(dc.tycx_ref());
            if !opt_def_tfp.is_some() { continue;}
            let def_tfp=opt_def_tfp.unwrap();
            let mut links_written=0 as uint;

            // sort references by file [(file_index, [...])]

//          let s2=refs_by(|x|{ let
//          let rf=refs.map(|x|{  })
//          let refs1:~[&ast::NodeId]=refs.iter().filter(|&x|{nim.find(x).is_some()}).collect();
//          let  refs1:() = refs.iter().filter(|&x|{true});

//          let  refs1:~[int] = refs.iter().filter(|x|{true}).collect();
//          dump!(refs1);
            // just cannot get filter working here :(


            let mut curr_file=def_tfp.file_index;
            let mut refs2=refs.iter()
                .filter(|&id|{nmaps.nim.find(id).is_some()})
                .map(|&id|{
                    let oni=nmaps.nim.find(&id); assert!(oni.is_some());
                    let ni=oni.unwrap();
                    let oifp=ni.span.lo.to_index_file_pos(dc.tycx_ref());
                    assert!(oifp.is_some()); let ifp=oifp.unwrap();
                    (ni,ifp,id)})
                .collect::<~[(&FNodeInfo,ZIndexFilePos,u32)]>();

            let l=refs2.len();
            fn pri_of(x:&FNodeInfo) -> uint { 
                if &"impl" == x.kind {
                    0
                } else {
                    0x8000
                }
            }
            // todo: we want to sort based on node type to find impls, but we dont quite find what we want..
            refs2.mut_slice_to(l - 1).sort_by(|&(ni1, ref ifp1, _), &(ni2, ref ifp2, _)| {
                use std::cmp::{Less, Greater};
                if ((ifp1.file_index - curr_file) & 0x7fff) as uint +
                        pri_of(ni1) <= ((ifp2.file_index - curr_file) & 0x7fff) as uint +
                        pri_of(ni2) {
                    Less
                } else {
                    Greater
                }
            });
            let mut newline=true;
            let mut last_link_line=0;
            for &(_, ref ref_ifp,ref id) in refs2.iter() {
                if *id!=dn {
                    //let opt_ref_info = nim.find(r);
                    //if !opt_ref_info.is_some() {loop;}
                    //let ref_info = opt_ref_info.unwrap();
                    //let opt_ref_tfp = byte_pos_to_index_file_pos(dc.tycx,ref_info.span.lo);
                    //if !opt_ref_tfp.is_some() {loop;}
                    //let ref_tfp=opt_ref_tfp.unwrap();

                    if header_written==false {
                        if newline==false {doc.writeln("");}
                        header_written=true;
                        doc.write_refs_header(dc,nmaps.nim,fm,dn);
                        doc.writeln_tagged("c40","references:-");
                        newline=true;
                    };
                    // Write a reference to the file, if we're looking a new file now
                    if curr_file!=ref_ifp.file_index {
                    	last_link_line=!0; // invalid value 
                        if newline==false {doc.writeln("");}
                        curr_file=ref_ifp.file_index;
                        doc.write_file_ref(dc,fm,curr_file as uint);
                        newline=true;
                    }

                    if links_written<(max_short_links+max_links) {
                        if  links_written<max_links {
                            if newline==false {
                                doc.writeln("");
                            }
                        }
                        let files = dc.codemap().files.borrow();
                        let rfm = &files.get(ref_ifp.file_index as uint);
                        doc.begin_tag_link(make_html_name_rel(rfm.name, fm.name) + "#" + (ref_ifp.line + 1).to_str());

                        if  links_written<max_links {
                        	// Display a line from the referenced location, but
                            // step past any #[lang items] - we want to show a meaningful definition.
                            let  mut ref_line_index = ref_ifp.line as uint;
                            let mut src_line=get_source_line(&***rfm, ref_line_index);
                            while ref_line_index < num_source_lines(&***rfm){ 
                            	src_line = get_source_line(&***rfm, ref_line_index);
                            	if src_line.chars().nth(0).unwrap_or('\0')!='#' {break};
                            	ref_line_index+=1;
                            }
                            if last_link_line !=ref_line_index { // dont write multiple links on the same line.
	                            doc.write_tagged("c40",(ref_ifp.line+1).to_str()+&": ");
                            	last_link_line=ref_line_index;
	                            doc.writeln(src_line);
	                            newline=true;
	                            links_written+=1;
						   }
                        } else {
                        	if last_link_line != ref_ifp.line as uint+1 {
	                            doc.write_tagged("c40","("+(ref_ifp.line+1).to_str()+")");
    	                        newline=false;
    	                        links_written+=1;
    	                        last_link_line=ref_ifp.line as uint+1;
							}
                        }
                        doc.end_tag();
                    }
                }
            }
            if links_written < refs2.len() {
                doc.begin_tag("c40").writeln(".."+(refs2.len()-links_written).to_str()+"more..").end_tag();
            }
            if header_written {doc.writeln("");}
    //      info=nim.find(dn);
        }
    }
    doc.end_tag();
    doc.check_depth(depth);
}

impl htmlwriter::HtmlWriter{
    fn write_refs_header(&mut self,dc:&RFindCtx,nim:&FNodeInfoMap, fm:&codemap::FileMap, nid:ast::NodeId) {
    	let depth=self.depth();
        self.writeln("");
        nim.find(&nid).for_some( |info| {
            let oifp=info.span.lo.to_index_file_pos(dc.tycx_ref());//.unwrap();
            let oifpe=info.span.hi.to_index_file_pos(dc.tycx_ref());//.unwrap();
            if (oifp.is_some() && oifpe.is_some())==true {
                let ifp=oifp.unwrap();
                let ifpe=oifp.unwrap();
    //      let def_info=nim.find(&nid).unwrap();
    //      let ifpe=get_node_index_file_pos(dc		,nim,nid).unwrap();

                self.begin_tag_anchor((ifp.line+1).to_str()+"_"+ifp.col.to_str() + "_refs" );
                self.begin_tag_link( "#"+(ifp.line+1).to_str());
                self.begin_tag("c40");
                self.writeln(dc.codemap().files.borrow().get(ifp.file_index as uint).name + ":" + (ifp.line + 1).to_str() + ":" + ifp.col.to_str()
                            +"-"+(ifpe.line+1).to_str()+":"+ifpe.col.to_str() +" -" +info.kind + "- definition:");
                self.end_tag();
                self.begin_tag("pr");
            //          dump!(def_tfp);
                self.writeln(get_source_line(fm,ifp.line as uint) );
                self.writeln(get_source_line(fm,ifp.line  as uint+1) );
                self.end_tag();
                self.end_tag();
                self.end_tag();
            }
        });
        self.check_depth(depth);
    }

    fn write_file_ref(&mut self, dc:&RFindCtx,origin_fm:&codemap::FileMap, fi:uint) {
        let fname = dc.codemap().files.borrow();
        let fname = fname.get(fi).name.as_slice();
        self
            .begin_tag_link( make_html_name_rel(fname,origin_fm.name));
        self
            .begin_tag("c40")
            .writeln(""+fname + ":")
            .end_tag();
        self
            .end_tag();
    }

    pub fn write_path_links(&mut self/*doc:&mut HtmlWriter*/, file_name:&str) {
    	let pldepth=self.depth();
        self.writeln("");
        let file_path_col=&"c0";
        let file_delim_col=&"c1";
        let name_parts = file_name.split('/').collect::<~[&str]>();
        let num_dirs=name_parts.len()-1;
        let mut link_target=~"./";


        for _ in range(0,num_dirs) {link_target.push_str("../");}
        self.write("    ");
        let t0=self.begin_tag_link(link_target+"index.html"); self.write("(index<- )"); self.end_tag_check(t0); self.write("    ");
        let t1=self.begin_tag_link(link_target); self.write("    ./"); self.end_tag_check(t1);

        for (i,x) in name_parts.iter().enumerate() {
            let is_dir = i < num_dirs;
            link_target.push_str(*x);
            if is_dir {link_target.push_str("/");}
            else { link_target.push_str(".html");}
            
            let fpc=self.begin_tag_check(file_path_col);
            let fpl=self.begin_tag_link(link_target); self.write(*x);
            self.end_tag_check(fpl);/*link*/
			self.end_tag_check(fpc);/*file_path_col*/
            if is_dir {self.write_tagged(file_delim_col,"/");}
        }
        self.writeln("");
        self.writeln("");
        self.check_depth(pldepth);
    }
}

// TODO: a span index should uniquely identify the node.
// This adress form is a reasonable compromise between things we can see,
// things that are robust when some source changes, etc.
// file_index:line_index:col_index:length

impl<'a> ToZIndexFilePos for (&'a FNodeInfoMap,ast::NodeId) {
    fn to_index_file_pos(&self,tc:&ty::ctxt)->Option<ZIndexFilePos> {
        let (ref nim,nid)=*self;
        let oni=nim.find(&nid);
        if oni.is_some() {
            let ni=oni.unwrap();
            ni.span.lo.to_index_file_pos(tc)
        } else {
            None
        }
    }
}

fn make_html_name(f: &str) -> ~str { 
    f + ".html"
}

fn count_chars_in(f:&str, x:char)->uint{
    let mut n=0;
    for c in f.chars() { if c==x {n+=1} }
    n
}


fn make_html_name_reloc(f:&str, origin:&str, reloc:&str)->~str {
    let mut acc=~"";
    if reloc.len()>0 {
        acc=reloc.to_owned();
        if acc.chars().last().unwrap()!='/' { acc.push_char('/');}
    }else {
        for _ in range(0,count_chars_in(origin,'/')) {
            acc.push_str("../");
        }
    }
    acc.push_str(f);
    make_html_name(acc)
}
fn make_html_name_rel(f:&str, origin:&str)->~str {
    make_html_name_reloc(f,origin,"")
}



