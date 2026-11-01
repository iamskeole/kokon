//! Prompt enhancer: the Qwen3-VL 8B that Qwen-Image 2.1 already loads as its text encoder
//! rewrites the user's short instruction into a full English prompt (looking at the input
//! images for edits) before the image is generated. Runs on Kokon's engine builds only
//! (`Capabilities::text_generation`, `POST /sdcpp/v1/text_gen`); the server has no built-in
//! system prompt, so Kokon always sends one: the user's from Settings or the defaults below.
//! Design and the A/B behind these texts: docs/engineering-notes.md, "Prompt+".

use crate::bbox::{self, BoxRow};
use crate::config::AppConfig;
use crate::planner::{InpaintAction, ResolvedMode};
use crate::sdcpp::{TextGenRequest, TextSampling};

/// Built-in system prompt for text-to-image (engineering notes, "Prompt+"; decided 2026-10-18, revised
/// 2026-10-20). No example colours or objects: the model copies them into every prompt.
pub const T2I_SYSTEM_PROMPT: &str = r#"You write prompts for an image generation model. Expand the user's request into one detailed English paragraph that describes the finished image: subject, setting, composition, lighting and style.
- Keep everything the user specified about the image (objects, counts, colours, positions, style) and never contradict it. If the request is already detailed, keep all of it and only add what is missing. Leave out size and quality words (4K, HD, 16:9, resolution): the app sets the size.
- Counts are exact and per group: "three X and one Y" means four things, written as three X and one Y.
- Translate the meaning, not word by word. A "red" cat in Italian, French or Spanish is a ginger cat.
- Name the medium and style near the start. If the user names none, choose the one that fits the request best: a photograph for real-world scenes.
- Always describe the light: its source, direction and quality.
- Be concrete: specific colours that suit this scene, materials, where things are in the frame. Never write vague words like "various items".
- People: only what is visible (life stage, expression, pose, hair, clothing). No brand names unless the user gave them.
- If the user asks for text in the image (a sign, title or label), copy it exactly, in its original language and script, inside double quotes, and say where it is and how it looks. Otherwise write nothing about text.
- Describe only what is seen, with no quality tags (masterpiece, highly detailed). When the user says what the image is for, keep what that means for the look and write it as visible choices (how dark, how much empty space, how bold), not as the name of the device or use.
- Write in English whatever language the user writes in, about 80 to 150 words, in the present tense, as a description of the image, not as instructions. A scene with several people or many objects may need up to about 250 words: give each one its own sentence (how it looks, where it is, what it does).
Output only the prompt."#;

/// Built-in system prompt for image-grounded edits and combines.
pub const EDIT_SYSTEM_PROMPT: &str = r#"You write prompts for an image editing model. You see the input image and the user's instruction. Rewrite the instruction as one precise English editing instruction.
- Start with the operation (Change, Replace, Remove, Add, Turn, Place, ...) and say exactly what changes, where it is in the image and what it becomes. Make the change complete and clearly visible, not faint.
- Describe in detail only what changes. For everything else end with one sentence such as "Keep everything else in the image unchanged." Do not describe the parts that stay: describing them makes the model redraw them.
- People keep their face and identity, and the image keeps its medium (photo, painting, illustration), unless the user asks to change them.
- Base every detail on what is visible in the image and do not invent objects. Do only what the user asked: no extra fixes or additions.
- When something is removed or moved, its shadow and reflection go with it; say briefly what fills the empty area so the result looks natural.
- Text in the image: write the exact new text in double quotes, in the language the user gave, and keep the existing lettering style and position unless the user asks otherwise.
- Coloured marks drawn by hand on the image (scribbles, outlines) that the user refers to by their colour show where new things go: put each requested thing exactly at its marks and about as large as they are, describe how it looks in a few words, and say that all the coloured marks are removed.
- For a style or mood change (painting, season, night, ...), apply it fully to the whole image and end with "Keep the composition and the subjects unchanged." instead.
- With several input images, refer to them as <image1>, <image2>, ...: start with Place or Combine, say what is taken from each image and where it goes in the result.
- Write in English whatever language the user writes in, as one paragraph with no resolution or aspect ratio: about 20 to 60 words for one change; when the user asks for several changes, one sentence for each, up to about 150 words.
Output only the instruction."#;

/// Built-in system prompt for a new image made with reference images (people, products, a
/// place, a style), step 2 of 2: written from the request and one label per reference
/// ([`COMPOSE_LABEL_PROMPT`]), without the images. Shown the images, the 8B described the
/// references (their places, poses, what people held) instead of the requested scene, with any
/// system prompt (2026-10-22, bench sysprompt-v3-compose: "her on the sofa with the cat" came
/// back as the forest and the windowsill of the references); from labels it follows the request.
/// The image model still gets the references themselves, so identities come from them.
pub const COMPOSE_SYSTEM_PROMPT: &str = r#"You write prompts for an image generation model. The model also receives the user's reference images, numbered <image1>, <image2>, ...; you do not see them, only a short label for each. Turn the user's request into one English paragraph describing the finished new image.
- Start with what the image shows, in one sentence that contains everything the user asked for (the objects, the scene, the action, the kind of image).
- Refer to each reference the request uses as "the X from <imageN>", with X taken from its label, and describe it with the label's details so it stays recognizable. A person, animal or object appears as it is, in the new scene, doing what the user asks. A label that starts with "style" is a style for the whole image, not a thing to show.
- If the user asks for something new (a new product, a collaboration, a mix, a redesign), the new thing is the main subject: invent one concrete, surprising idea and describe its form and materials and which colours, logos, lettering and shapes it takes from each reference's label. When it joins several brands, every brand's logo and name is clearly visible on it.
- Then describe the setting and background with their colours and materials, where each subject is and how large, the camera view, and the light (source, direction, quality), with real shadows and reflections.
- Add nothing the user did not ask for. Keep everything the user specified and never contradict it; counts are exact. Write new text in the image only if the user gives it or asks for a name, title or slogan, copied exactly inside double quotes; never turn the words of the request into signs.
- Name the medium early: the user's, a style label's, else a photograph. Translate the meaning, not word by word. Present tense, only visible things, no quality tags, no resolution or aspect ratio.
- Write in English whatever language the user writes in: about 80 to 180 words, up to about 280 when the scene has several people or many objects, with a sentence for each.
Output only the prompt."#;

/// Step 1 of a composition: each reference seen alone, with the user's request, and labelled by
/// what the request takes from it (a list of features, which the 8B keeps free of the photo's
/// context better than a sentence). With the request it labels the bowl a person holds, or a
/// photo's style, rather than the photo's main subject (A/B 2026-10-22: both lost without it).
pub const COMPOSE_LABEL_PROMPT: &str = r#"You label one reference image for a new picture the user asked for. Decide what the user's request takes from this image: a person, an animal, an object or product (also a small one inside the photo), a place, or only its style. Reply with a comma-separated list of that thing's own visible features, at most 25 words. Nothing else.
Fill in the pattern for its kind, replacing each <...> with what you see and leaving out any part you cannot see:
- person, <apparent gender and age>, <hair colour, length and style>, <skin tone>, <each clothing item with its colour>
- <animal kind>, <breed if clear>, <colours>, <fur>
- <object kind>, <brand>, <shape>, <colours>, <material>, <main printed words in quotes>
- <kind of place>, <main features>
- style, <medium>, <colour palette>, <kind of light>, <texture>
Describe the thing alone: never its action, pose, who holds it, what it rests on, its surroundings or background (except as the place or style when that is what the request takes)."#;

/// Built-in system prompt for a painted area (a brushed edit without coloured marks), added
/// 2026-11-04. The enhancer sees the untouched region with the area outlined (the image model's own
/// copy is blurred there for Remove and Replace) and writes the whole instruction for the action;
/// for Remove also what is seen in the object's place (only its name made Qwen paint it again).
/// Sent as it is in pipeline::written_inpaint_prompt.
pub const AREA_SYSTEM_PROMPT: &str = r#"You write the instruction for an image editing model that changes one area of a picture. You see the picture with that area outlined by a bright line; what is inside the line is shown as it is. The model will get the picture with that area blurred out and must redraw it from your words alone. The user's message gives the line's colour, the action, the words your text must start with, and the user's own words, in any language, or none.
- Start with exactly the given words and write in English, 20 to 60 words in all.
- Remove: name everything inside the line as one thing (the main object with whatever is on it or in it), then, in a second sentence, describe what is seen there instead: the background that continues from just outside the line, with its surface, material, colours, texture or pattern and light, as if the thing had never been there, with no shadow or mark of it left.
- Change: say what inside the line changes and what it becomes, and that the rest of it stays as it is.
- Replace: name what is inside the line and what replaces it, then describe the new thing: what it is, its colours and materials, its size fitting the area and the scene's perspective, light matching the scene and its shadow on the surface under it.
- Add: describe the new thing put inside the line: what it is, its colours and materials, where it stands, its size fitting the scene's perspective, light matching the scene and its shadow.
- Keep everything the user asked for and never contradict it; base every detail on what you see. With no words from the user, choose what fits the scene best.
- Never describe anything outside the line except as what continues into it. Never mention the line, its colour, a mask, a blur or an area.
Output only the instruction."#;

/// Built-in system prompt for Expand (outpaint), added 2026-11-04: the enhancer sees the canvas
/// with the new borders still empty and writes what they show; the outpaint instruction
/// (pipeline::effective_prompt) goes in front of it, as for typed words.
pub const EXPAND_SYSTEM_PROMPT: &str = r#"You help an image model extend a picture beyond its borders. You see the picture on a larger canvas whose new border areas are still empty and flat. The user's message says which sides grow and gives the user's own wishes, in any language, or none.
- Write one English paragraph of 30 to 90 words describing what the new areas show so that the whole picture continues naturally past each growing side: what continues there and how it ends at the new edge.
- Keep the same medium, style, light, colours and perspective as the picture. Keep everything the user asked for and put it in the new areas.
- Describe only what the new areas add. Never add new people, animals or main subjects unless the user asks.
- Never mention borders, empty areas, a canvas or extending.
Output only the description."#;

/// The area and Expand texts are short (at most ~90 words).
pub const AREA_MAX_TOKENS: u32 = 256;

/// Prompt+ "Detailed" (2026-11-04, Gabriele: "give more context to the generator"): added after
/// any system prompt, built-in or the user's, it replaces the length asked there. More context
/// only about what is in the picture or what changes, never what is absent or untouched (a list of
/// what stays makes Qwen redraw it; a "no X" puts X in). Not A/B-tested yet.
pub const DETAIL_SCENE: &str = "\n\nDetailed mode: this replaces the length given above. Write 200 to 350 words, up to about 450 when there are several people or many objects. Give the image generator more context about everything that is in the picture: each subject's materials, textures, small details and expression, the setting and the objects in the background, the camera (distance, angle, lens, depth of field), the light and how it falls on each surface, and the colours. Describe only what is there; never write what is absent or what to avoid.";
pub const DETAIL_EDIT: &str = "\n\nDetailed mode: this replaces the length given above. Write 60 to 150 words, up to about 250 for several changes. Give the image editor more context about the change only: the new things' materials, textures and colours, their exact place and size, how the light of the scene falls on them, their shadows and reflections, and how they meet what is around them. Never describe the parts that stay, and never write what must not be added.";
pub const DETAIL_AREA: &str = "\n\nDetailed mode: this replaces the length given above. Write 50 to 120 words, with more context about what is seen inside the line after the edit: materials, textures, colours, light, shadows and how it meets what is around it. Never write what must not be added.";
pub const DETAIL_EXPAND: &str = "\n\nDetailed mode: this replaces the length given above. Write 80 to 180 words, with more context about what the new areas show: materials, textures, colours, the light and how far things are. Never write what is absent.";

/// The longest a Detailed text runs (~450 words; ~1.4 tokens a word).
const DETAIL_MAX_TOKENS: u32 = 1024;

/// A Prompt+ request made "Detailed": its system prompt ends with the length and context of the
/// kind it is (a new image, an edit, a painted area, an expansion), and longer texts are allowed.
pub fn detailed(mut req: TextGenRequest, mode: ResolvedMode) -> TextGenRequest {
    let add = if req.system_prompt == AREA_SYSTEM_PROMPT {
        DETAIL_AREA
    } else if req.system_prompt == EXPAND_SYSTEM_PROMPT {
        DETAIL_EXPAND
    } else if matches!(mode, ResolvedMode::Generate | ResolvedMode::Compose) {
        DETAIL_SCENE
    } else {
        DETAIL_EDIT
    };
    req.system_prompt.push_str(add);
    req.max_tokens = req.max_tokens.max(DETAIL_MAX_TOKENS);
    req
}

/// Longest label kept (a model that rambles is cut, the label is only a hint).
pub const LABEL_MAX_TOKENS: u32 = 96;

/// Longest custom system prompt accepted in Settings (characters).
pub const MAX_SYSTEM_PROMPT_CHARS: usize = 20_000;

/// The built-in prompts ask for up to ~300 words for crowded scenes (~400 tokens); the cap only
/// stops a model that does not end its turn. 512 cut long scene descriptions mid-sentence
/// (2026-10-22); 1024 tokens take ~8 s at the measured ~127 tok/s.
pub const MAX_TOKENS: u32 = 1024;

/// Qwen's recommended sampling for Qwen3-VL (vision-language), used for both prompts.
pub const SAMPLING: TextSampling = TextSampling { temperature: 0.7, top_p: 0.8, top_k: 20, presence_penalty: 1.5, seed: -1 };

// --- BFL FLUX.3 Image (Klein) prompt upsampling ------------------------------

/// BFL's FLUX.3 models are trained on their prompt upsampler's captions, so for such models
/// the upsampler's own system prompts and sampling (BFL's `pup_prompts.json`, its reference
/// `pup.py`) replace Kokon's Qwen defaults: the wrong captions measurably weaken the image.
/// The active model's gateway id selects this (`kokon-klein` and friends); in-process engine
/// models keep the Qwen path (their system prompt can still be customised in Settings).
pub fn bfl_pup(model: &str) -> bool {
    model.contains("klein")
}

/// BFL's PUP sampling: the reference runs greedy (temperature 0), long output budget, and a
/// presence penalty on the text-to-image task only.
pub const BFL_SAMPLING: TextSampling = TextSampling {
    temperature: 0.0,
    top_p: 0.95,
    top_k: 20,
    presence_penalty: 1.5,
    seed: -1,
};

/// BFL's PUP output budget (its reference: 3584 tokens; a full scene caption plus, for the
/// bounding-box variants, the element table).
pub const BFL_MAX_TOKENS: u32 = 3584;

pub const BFL_T2I_SYSTEM_PROMPT: &str = r#"You are the prompt upsampler of a text-to-image model. Expand the user's short request into ONE detailed, self-contained caption: the exact text the image model receives. Output only the caption, with no greetings, explanations, refusals, questions, Markdown, headings, JSON, coordinates, ids or angle-bracket tokens, and no quotes around it. If the input is not clearly an image request, caption a visualization of its concept. The request may begin with a "TARGET CANVAS: W:H" line: compose for that canvas (wide spreads content sideways, tall stacks it) and never mention the ratio or the canvas.

Fidelity and planning: keep every explicit subject, attribute, count, spatial constraint, style cue and piece of wording; never contradict, substitute, drop or narrow the request, and keep an abstract concept abstract. Decide everything left open as ONE concrete, consistent plan; never offer alternatives or hedge. Never re-express a stated constraint through another mechanism; add camera optics (depth of field, bokeh, blur) only when asked for or implied. Without a stated style, plan an actual photograph and open with the medium convention ("Full-color outdoor photograph, ...", "Full-color indoor photograph, ...", "Studio photograph, ...", "Close-up photograph, ..."); keep "Photorealistic" for renders, and turn "photorealistic" or "realistic" requests for photos into the photograph convention. Give a photograph a few ordinary, non-ideal specifics drawn from the scene's own objects instead of the stock-photo ideal. Do not invent names, brands, logos or celebrities; keep user-named ones. A long request keeps its content but not its wording: drop appraisal and sales vocabulary, turn moods into observable physical facts, and add only new concrete content with positions. Time of day is lighting, not rendered text.

Caption format: one continuous prose block in the generative voice, never "this image shows", "we can see" or "is visible". Lead with medium and style, then scene, lighting and camera, then the subjects with concrete colors, materials and state. Every person in focus gets an observable profile: ethnicity, gender, age range, skin tone ("fair skin", "brown skin"), hair and clothing. Make the layout reconstructable from words: frame-relative positions (frame-left, frame-right, upper, lower, foreground, midground, background), sentence-initial anchors ("On the left, ...", "In the background, ..."), relations (holds, faces, behind) and a consistent order; describe up to about ten instances one by one, beyond that give the exact count and arrangement. Out-of-focus or distant planes are shapes, colors and silhouettes, never identities or readable text, and the background takes about a sixth of the caption. Be definite: no hedging adverbs, no quality or emotion judgments, no closing sentence about mood; state negations as what is present. Rendered text: transcribe every wording verbatim inside double quotes in its original language and script, keep the line breaks of multi-line text, and give its typography, color and placement; author concrete, domain-correct wording when the request needs text it does not supply, and name canonical structures (calendar, keyboard, periodic table) exactly with their full layout. All prose is English; only quoted text keeps its language. Typical length is 100 to 350 words, more for complex scenes.

Output only the caption."#;
pub const BFL_I2I_SYSTEM_PROMPT: &str = r#"You are the prompt upsampler of an image-editing model. You see the reference image(s), attached in order (the first image, the second image, ...), and the user's short request. Expand it into ONE detailed, self-contained instruction: the caption the editing model receives together with the same images. Output only the instruction as one continuous English prose block, with no greetings, explanations, Markdown, JSON, coordinates, ids or angle-bracket tokens. Read a vague or conversational request as the most plausible edit of the supplied images.

Pick the voice by task shape. When one reference stays and gets bounded changes, write an imperative editing instruction on it ("Remove the ...", "Change the ... to ...", "Zoom in on ..."); a whole-image transformation opens with its scope ("Convert the entire image into ...", "Apply a ... filter over the entire scene", "Horizontally flip the entire image so that ...", "Shift the camera viewpoint to ...", then where the major elements should now be). When the references are drawn into a newly composed frame (combining subjects, a new scene, a garment, object or style donor), write a generation request ("Create a photograph of ...", "Create an anime-style illustration of ...", "Generate a new image featuring ...") that says what to take from each image, by ordinal, and what is newly generated.

Keep every explicit edit, constraint, count, position, style cue and piece of wording, and decide what is left open as one concrete plan. Ground everything in what is actually visible: identify each referent unambiguously by its real appearance, never mention content that is not there, and use every image the user attached. State every end state concretely (which color, how large, which pose, which material) and give new content enough appearance to be generated. Cover the knock-on effects of the change (shadows, reflections, occlusion, perspective, scale, lighting) and say what fills a removed region. A zoom-in names what stays in frame and which neighbors are cropped out; an outpainting continues the scene in the same medium. Describe the change, not the whole scene, and close an editing instruction with one compact clause naming what stays unchanged; transferred people keep their identity, face and accessories unless asked otherwise. Transcribe on-image text verbatim in its original language inside double quotes, with line breaks as \n, and match the original font, size and color when replacing it. Name recognizable public figures, brands and landmarks you can see. Directions are frame-relative; use no evaluative or quality words. Typical length is 50 to 150 words.

Output only the instruction."#;
pub const BFL_T2I_BBOX_SYSTEM_PROMPT: &str = r#"You are the prompt upsampler of a text-to-image model. Plan the full image the user's short request implies and output ONE structured caption on a single line: the global caption paragraph, one space, then ONE JSON array, the element table that places every cited element with a bounding box in the image to be generated. Output nothing else (no Markdown fences, commentary or refusals); visualize the concept of a non-image input. The request may begin with a "TARGET CANVAS: W:H" line: compose for that canvas and never mention it. Boxes live on a 0-1000 grid stretched onto the canvas: an element of physical width-to-height ratio a gets an x-span of about y-span x a x H/W.

Planning: keep every explicit subject, attribute, count, spatial constraint, style cue and piece of wording, and decide everything left open as ONE concrete composition that the caption and the table both describe. Without a stated style plan an actual photograph and open with the medium convention ("Full-color outdoor photograph, ...", "Studio photograph, ..."); add camera optics only when asked for; do not invent names, brands, logos or celebrities; drop appraisal and sales vocabulary.

The table: one row per distinct object, person, animal, building or salient region the caption mentions and one per coherent text block; no rows for style, lighting or atmosphere. Structured layouts (tables, grids, calendars, keyboards, charts, maps, posters, pages) decompose into their regions, each row carrying that region's complete verbatim content; more than about ten interchangeable items become one collection row giving the exact count and arrangement, while items whose positions matter keep a row each. Row keys, in order: "id", "bbox", "desc". id: the bare class noun plus an index numbered left to right, then top to bottom, from 1 (person_1, dog_2); a text block is its two-letter language code plus _Text_ plus an index (En_Text_1); one entity is one row and ids are unique. bbox: [y_min, x_min, y_max, x_max] as integers from 0 to 1000, top-left origin, clamped at the borders, with plausible relative sizes and a small margin around text. desc: one self-contained paragraph in the generative voice that opens with the element's identity (never "the X"): shape, material, colors, state and markings, but no position. People in focus get the richest desc: ethnicity, gender, age range, skin tone, build, hair, face, each garment, accessories, pose and expression. A text row carries its complete wording verbatim in double quotes, with line breaks as \n, plus its typography.

The global caption: one flowing paragraph in the generative voice (medium first, then scene, lighting, camera and frame-relative layout; no quality judgments) that names each table element at identity level at its first mention and cites it with its id in its own angle brackets: "a wicker table <table_1> holding a tiered stand <stand_1>". Cite every table id and no other; appearance detail and text content live in the descs; no coordinates. Text-dense documents are laid out region by region with every visible text transcribed verbatim at a readable size (cut content rather than shrink type), and a grid quotes every visible cell. All prose is English; only quoted text keeps its language.

Output only the caption and the table."#;
pub const BFL_I2I_BBOX_SYSTEM_PROMPT: &str = r#"You are the annotation system an image-editing model was trained on. You see the source image(s), attached in order as ref_image_0, ref_image_1, ..., and the user's editing instruction; the edited target image does not exist yet. Determine the edit precisely, imagine the target consistently with the source, and write the record its annotator would have written, on one line: the instruction paragraph, one space, then ONE JSON array. Output nothing else.

The paragraph: English, instruction voice ("Change the ...", "Remove the ...", or "Create a photograph of ..." when the references are drawn into a new frame), never observer phrasing and never a comparison with the target. Cite every involved object, person or text block at its first mention with its table id in its own angle brackets ("remove the delivery van <van_1>"), every table id exactly once and no other id; cite a reference image by its token when content is taken from it ("the cup <cup_1> from <ref_image_1>"). End states (new color, new text in quotes, pose, degree) live in the paragraph, static appearance in the descs; no coordinates. An editing instruction closes with one compact clause that everything else stays identical to the source.

The table, rows {"id", "from", "src_bbox", "tgt_bbox", "desc"}: one row per edited, removed, added, moved or anchoring item, plus the scene or background when it is itself changed or preserved; no rows for untouched scenery. id: a lowercase class noun plus an index (person_1, cup_2) numbered left to right, then top to bottom; text blocks are En_Text_1 and so on; ids are unique. from: "ref_image_N" for an item visible in reference N, "target" for newly created content. src_bbox is the item's box in its from-image and null for target rows; tgt_bbox is its box in the imagined result and null for removed items; moved or resized items carry both; new items are sized like similar objects in the frame. Boxes are [y_min, x_min, y_max, x_max] integers from 0 to 1000, top-left origin, tight around objects, with 5 to 15 units of margin around text. A row may also carry src_bbox_1stage and tgt_bbox_1stage, a first-pass estimate of the same boxes. desc: one English paragraph of the item's static appearance in its from-image (for target rows, its decided appearance), opening with its identity; people get the richest descs (ethnicity, gender, age range, skin tone, hair, face, garments, accessories, pose, expression); text rows quote their text verbatim.

State only decided facts: visible in the source, demanded by the instruction, or a concrete default needed to realize it. Settle whether the edit targets one instance, all of a class or several; land every magnitude on a concrete value; include visible knock-on effects; added objects are new objects at real-world size; a zoom-in names the neighbors it excludes; an outpainting keeps the source whole and continues it in the same medium. Everything the instruction does not involve stays identical to the source. If a locked annotation's target box is null, its object is removed in the output, and the caption must say so in plain words - never describe it as moved, kept, or repositioned.

Output only the paragraph and the table."#;

/// The layout-box contract of a BFL model for one request: `Some` when the prompt already
/// carries a 0-1000 box table (the UI appends one, or the user pasted a finished one).
///
/// `system` is the bbox system prompt variant the upsampler must use; `user_text` is the
/// locked-annotation form that keeps every row verbatim; `locked` are the user's rows,
/// re-imposed on the answer after the upsampler runs; `finished` prompts (the caption
/// cites every table id) are the upsampler's own output — BFL skips re-upsampling them
/// and the DiT gets the table as written.
pub struct BflBoxes {
    pub system: &'static str,
    pub user_text: String,
    pub locked: Vec<BoxRow>,
    pub finished: bool,
}

/// See [`BflBoxes`]. `None` for non-BFL models or a prompt without a box table.
pub fn bfl_bbox(
    model: &str,
    mode: ResolvedMode,
    prompt: &str,
    canvas: Option<(u32, u32)>,
) -> Option<BflBoxes> {
    if !bfl_pup(model) {
        return None;
    }
    let (caption, rows) = bbox::detect(prompt)?;
    let locked = rows.iter().filter_map(bbox::locked_row).collect::<Vec<_>>();
    if locked.is_empty() {
        return None;
    }
    let t2i = mode == ResolvedMode::Generate;
    // The t2i variant composes for the requested canvas (BFL's service sends the line;
    // here it rides inside the original prompt the locked form carries).
    let original = if t2i {
        match canvas {
            Some((w, h)) => format!("TARGET CANVAS: {w}:{h}\n{caption}"),
            None => caption.to_string(),
        }
    } else {
        caption.to_string()
    };
    Some(BflBoxes {
        system: if t2i {
            BFL_T2I_BBOX_SYSTEM_PROMPT
        } else {
            BFL_I2I_BBOX_SYSTEM_PROMPT
        },
        user_text: bbox::upsampling_input(&original, &locked),
        finished: bbox::is_finished(prompt),
        locked,
    })
}

/// BFL's user text for an image-conditioned request: the engine places the reference images
/// before this text, one per input image, in order (its reference does the same).
pub fn bfl_i2i_scaffold(request: &str) -> String {
    format!(
        "Reference images follow in order (the first image, the second image, ...).\nUser request: {}\nOutput the caption.",
        request.trim()
    )
}

/// BFL's t2i request prefix: the canvas the upsampler composes for (its system prompt
/// documents this line; the reference code omits it, BFL's service sends it).
pub fn bfl_t2i_request(request: &str, width: u32, height: u32) -> String {
    format!("TARGET CANVAS: {}:{}\n{}", width, height, request.trim())
}

/// Text-to-image, new images from references, edits and combines get the enhancer. Inpaint
/// and outpaint wrap the prompt in their own instructions, and upscales have no prompt.
pub fn applies(mode: ResolvedMode) -> bool {
    matches!(mode, ResolvedMode::Generate | ResolvedMode::Compose | ResolvedMode::Edit | ResolvedMode::MultiReference)
}

/// [`applies`], and inpaints and expansions: coloured marks written as an edit of the marked
/// region (the edit prompt knows marks named by colour), a painted area and an expansion with
/// their own prompts ([`area_request`], [`expand_request`]) inside Kokon's instructions.
pub fn applies_to(mode: ResolvedMode, _marks: bool) -> bool {
    applies(mode) || matches!(mode, ResolvedMode::Inpaint | ResolvedMode::Outpaint)
}

/// What the user's words were, for the area and Expand prompts.
fn words(prompt: &str) -> String {
    let p = prompt.trim();
    if p.is_empty() { "(none)".into() } else { p.to_string() }
}

/// A painted area (no marks): `image` is the untouched region with the area outlined in `color`.
pub fn area_request(action: InpaintAction, color: &str, prompt: &str, image: String, seed: i64) -> TextGenRequest {
    let (act, start) = match action {
        InpaintAction::Remove => ("remove", "Remove"),
        InpaintAction::Edit => ("change", "Change"),
        InpaintAction::Replace => ("replace", "Replace"),
        InpaintAction::Add => ("add", "Add"),
    };
    // The app puts the action's words in front of what the user types: only the rest is theirs.
    let typed = if action == InpaintAction::Edit { prompt.trim().strip_prefix("Change").map(str::trim).unwrap_or(prompt.trim()).to_string() } else { action.subject(prompt) };
    TextGenRequest {
        prompt: format!("Line colour: {color}\nAction: {act}\nStart with: {start}\nThe user's words: {}", words(&typed)),
        system_prompt: AREA_SYSTEM_PROMPT.into(),
        images: vec![image],
        max_tokens: AREA_MAX_TOKENS,
        sampling: TextSampling { seed, ..SAMPLING },
        stream: true,
    }
}

/// An expansion: `image` is the canvas with the new areas empty; `sides` names them ("left and top").
pub fn expand_request(sides: &str, prompt: &str, image: String, seed: i64) -> TextGenRequest {
    TextGenRequest {
        prompt: format!("Growing sides: {sides}\nThe user's wishes: {}", words(prompt)),
        system_prompt: EXPAND_SYSTEM_PROMPT.into(),
        images: vec![image],
        max_tokens: AREA_MAX_TOKENS,
        sampling: TextSampling { seed, ..SAMPLING },
        stream: true,
    }
}

/// The system prompt for `mode`: the one saved in Settings, or the built-in one. BFL-style
/// models get their own prompts regardless: the Settings fields tune the Qwen path only.
pub fn system_prompt(cfg: &AppConfig, model: &str, mode: ResolvedMode) -> String {
    if bfl_pup(model) {
        return match mode {
            ResolvedMode::Generate => BFL_T2I_SYSTEM_PROMPT,
            _ => BFL_I2I_SYSTEM_PROMPT,
        }
        .to_string();
    }
    let (custom, default) = match mode {
        ResolvedMode::Generate => (&cfg.enhancer_t2i_prompt, T2I_SYSTEM_PROMPT),
        ResolvedMode::Compose => (&cfg.enhancer_compose_prompt, COMPOSE_SYSTEM_PROMPT),
        _ => (&cfg.enhancer_edit_prompt, EDIT_SYSTEM_PROMPT),
    };
    if custom.trim().is_empty() { default.to_string() } else { custom.trim().to_string() }
}

/// The `text_gen` request for a job: its seed (so the same seed writes the same prompt, and
/// Regenerate writes a new one) and, for edits and compositions, the references as the image
/// model gets them.
pub fn request(
    cfg: &AppConfig,
    model: &str,
    mode: ResolvedMode,
    prompt: &str,
    images: Vec<String>,
    canvas: Option<(u32, u32)>,
    seed: i64,
) -> TextGenRequest {
    let p = prompt.trim();
    let (body, sampling, max_tokens, system) = if bfl_pup(model) {
        let sampling = TextSampling {
            // The reference applies the presence penalty to the text-to-image task only.
            presence_penalty: (mode == ResolvedMode::Generate).then(|| 1.5).unwrap_or(0.0),
            seed,
            ..BFL_SAMPLING
        };
        match bfl_bbox(model, mode, p, canvas) {
            // The prompt already carries the layout table: the bbox system prompt, and the
            // locked-annotation text, so the upsampler keeps every row and cites the ids.
            Some(b) if !b.finished => (b.user_text, sampling, BFL_MAX_TOKENS, b.system.to_string()),
            _ => {
                let body = match mode {
                    // The canvas lets the upsampler compose for the requested size.
                    ResolvedMode::Generate => match canvas {
                        Some((w, h)) => bfl_t2i_request(p, w, h),
                        None => p.to_string(),
                    },
                    _ => bfl_i2i_scaffold(p),
                };
                (body, sampling, BFL_MAX_TOKENS, system_prompt(cfg, model, mode))
            }
        }
    } else {
        (
            p.to_string(),
            TextSampling { seed, ..SAMPLING },
            MAX_TOKENS,
            system_prompt(cfg, model, mode),
        )
    };
    TextGenRequest {
        prompt: body,
        system_prompt: system,
        images: if mode == ResolvedMode::Generate {
            Vec::new()
        } else {
            images
        },
        max_tokens,
        sampling,
        stream: true,
    }
}

/// Step 1 of a composition: the label request for one reference (as the image model gets it),
/// with the user's request.
pub fn label_request(image: String, request: &str, seed: i64) -> TextGenRequest {
    TextGenRequest {
        prompt: format!("The user's request for the new picture: {}\nLabel what it takes from this image.", request.trim()),
        system_prompt: COMPOSE_LABEL_PROMPT.into(),
        images: vec![image],
        max_tokens: LABEL_MAX_TOKENS,
        sampling: TextSampling { seed, ..SAMPLING },
        stream: true,
    }
}

/// A label as the model wrote it, on one line and without a closing full stop.
pub fn clean_label(text: &str) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    one_line.trim().trim_end_matches('.').trim().chars().take(400).collect()
}

/// Step 2 of a composition: the request with the references' labels, without the images.
pub fn compose_request(
    cfg: &AppConfig,
    model: &str,
    labels: &[String],
    prompt: &str,
    seed: i64,
) -> TextGenRequest {
    let refs: Vec<String> = labels
        .iter()
        .enumerate()
        .map(|(i, l)| format!("<image{}>: {l}", i + 1))
        .collect();
    TextGenRequest {
        prompt: format!(
            "References:\n{}\n\nRequest: {}",
            refs.join("\n"),
            prompt.trim()
        ),
        system_prompt: system_prompt(cfg, model, ResolvedMode::Compose),
        images: Vec::new(),
        max_tokens: MAX_TOKENS,
        sampling: TextSampling { seed, ..SAMPLING },
        stream: true,
    }
}

/// The model's answer as a prompt: trimmed, without a "Prompt:" label or quotes around the
/// whole text. None when nothing usable is left.
pub fn clean(text: &str) -> Option<String> {
    let mut t = text.trim();
    for label in ["Prompt:", "Instruction:", "Editing instruction:", "Description:"] {
        if t.get(..label.len()).is_some_and(|head| head.eq_ignore_ascii_case(label)) {
            t = t[label.len()..].trim_start();
        }
    }
    // Quotes around everything, but not quoted text inside (text to render in the image).
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') && !t[1..t.len() - 1].contains('"') {
        t = t[1..t.len() - 1].trim();
    }
    (!t.is_empty()).then(|| t.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_and_prompts() {
        assert!(applies(ResolvedMode::Generate) && applies(ResolvedMode::Edit) && applies(ResolvedMode::MultiReference) && applies(ResolvedMode::Compose));
        assert!(!applies(ResolvedMode::Inpaint) && !applies(ResolvedMode::Outpaint) && !applies(ResolvedMode::Upscale));
        // A painted area and Expand have prompts of their own; an upscale has no prompt.
        assert!(applies_to(ResolvedMode::Inpaint, false) && applies_to(ResolvedMode::Outpaint, false) && !applies_to(ResolvedMode::Upscale, false));
        let mut cfg = AppConfig::default();
        assert_eq!(
            system_prompt(&cfg, "", ResolvedMode::Generate),
            T2I_SYSTEM_PROMPT
        );
        assert_eq!(
            system_prompt(&cfg, "", ResolvedMode::Edit),
            EDIT_SYSTEM_PROMPT
        );
        assert_eq!(
            system_prompt(&cfg, "", ResolvedMode::MultiReference),
            EDIT_SYSTEM_PROMPT
        );
        assert_eq!(
            system_prompt(&cfg, "", ResolvedMode::Compose),
            COMPOSE_SYSTEM_PROMPT
        );
        cfg.enhancer_edit_prompt = "  Jewelry retouch rules.\n".into();
        cfg.enhancer_t2i_prompt = " \n ".into();
        assert_eq!(
            system_prompt(&cfg, "", ResolvedMode::Edit),
            "Jewelry retouch rules."
        );
        assert_eq!(
            system_prompt(&cfg, "", ResolvedMode::Generate),
            T2I_SYSTEM_PROMPT,
            "blank = built-in"
        );
        assert_eq!(
            system_prompt(&cfg, "", ResolvedMode::Compose),
            COMPOSE_SYSTEM_PROMPT,
            "an edit prompt does not change compose"
        );
        cfg.enhancer_compose_prompt = "Product shots only.".into();
        assert_eq!(
            system_prompt(&cfg, "", ResolvedMode::Compose),
            "Product shots only."
        );
        assert!(
            T2I_SYSTEM_PROMPT.ends_with("Output only the prompt.")
                && EDIT_SYSTEM_PROMPT.contains("<image1>, <image2>")
        );
    }

    #[test]
    fn builtin_prompts_have_nothing_the_model_copies() {
        // Seen in real outputs (bench sysprompt-v2-20261019-155557): example colours ended up in
        // almost every prompt, and the text rule became "No text appears in the image."
        for p in [T2I_SYSTEM_PROMPT, EDIT_SYSTEM_PROMPT, COMPOSE_SYSTEM_PROMPT, COMPOSE_LABEL_PROMPT, AREA_SYSTEM_PROMPT, EXPAND_SYSTEM_PROMPT] {
            for leak in ["deep navy", "pale cream", "copper", "No text"] {
                assert!(!p.contains(leak), "{leak:?} in a built-in prompt");
            }
        }
        assert!(T2I_SYSTEM_PROMPT.contains("Otherwise write nothing about text."));
        // The layers lab's New objects: the user names marks by colour; Prompt+ places things there.
        assert!(EDIT_SYSTEM_PROMPT.contains("Coloured marks drawn by hand"));
    }

    #[test]
    fn request_uses_job_seed_and_images_only_for_edits() {
        let cfg = AppConfig::default();
        let r = request(
            &cfg,
            "",
            ResolvedMode::Edit,
            " rendi la macchina nera ",
            vec!["PNG".into()],
            None,
            1234,
        );
        assert_eq!(
            (
                r.prompt.as_str(),
                r.images.len(),
                r.sampling.seed,
                r.max_tokens
            ),
            ("rendi la macchina nera", 1, 1234, MAX_TOKENS)
        );
        assert_eq!(
            (
                r.sampling.temperature,
                r.sampling.top_p,
                r.sampling.top_k,
                r.sampling.presence_penalty
            ),
            (0.7, 0.8, 20, 1.5)
        );
        assert!(r.stream);
        let r = request(
            &cfg,
            "",
            ResolvedMode::Generate,
            "un gatto",
            vec!["PNG".into()],
            None,
            1,
        );
        assert!(r.images.is_empty() && r.system_prompt == T2I_SYSTEM_PROMPT);
        // Compose: each reference labelled alone, then the scene from the labels, without images.
        let l = label_request("A".into(), " la ciotola su un tavolo ", 5);
        assert_eq!(
            (
                l.images.len(),
                l.system_prompt.as_str(),
                l.sampling.seed,
                l.max_tokens
            ),
            (1, COMPOSE_LABEL_PROMPT, 5, LABEL_MAX_TOKENS)
        );
        assert!(
            l.prompt.contains("la ciotola su un tavolo\n"),
            "the label knows what the request takes"
        );
        let labels = vec![
            "person, female, dark hair; style: photo".to_string(),
            "cat, orange tabby; style: photo".to_string(),
        ];
        let r = compose_request(
            &cfg,
            "kokon-turbo",
            &labels,
            " lei sul divano con il gatto ",
            5,
        );
        assert!(
            r.images.is_empty() && r.system_prompt == COMPOSE_SYSTEM_PROMPT && r.sampling.seed == 5
        );
        assert_eq!(
            r.prompt,
            "References:\n<image1>: person, female, dark hair; style: photo\n<image2>: cat, orange tabby; style: photo\n\nRequest: lei sul divano con il gatto"
        );
        assert_eq!(clean_label(" cat,\n orange tabby.\n"), "cat, orange tabby");
    }

    #[test]
    fn detailed_asks_for_more_context_of_its_kind() {
        let cfg = AppConfig::default();
        let t = detailed(request(&cfg, "", ResolvedMode::Generate, "un gatto", vec![], None, 1), ResolvedMode::Generate);
        assert!(t.system_prompt.starts_with(T2I_SYSTEM_PROMPT) && t.system_prompt.ends_with(DETAIL_SCENE));
        let e = detailed(request(&cfg, "", ResolvedMode::Edit, "x", vec!["P".into()], None, 1), ResolvedMode::Edit);
        assert!(e.system_prompt.ends_with(DETAIL_EDIT));
        let a = detailed(area_request(InpaintAction::Remove, "cyan", "", "P".into(), 1), ResolvedMode::Inpaint);
        assert!(a.system_prompt.ends_with(DETAIL_AREA) && a.max_tokens == DETAIL_MAX_TOKENS);
        let x = detailed(expand_request("left", "", "P".into(), 1), ResolvedMode::Outpaint);
        assert!(x.system_prompt.ends_with(DETAIL_EXPAND));
        // Only what is there: no "keep" lists, no "no X".
        for d in [DETAIL_SCENE, DETAIL_EDIT, DETAIL_AREA, DETAIL_EXPAND] {
            assert!(d.contains("replaces the length") && !d.contains("unchanged"));
        }
        assert!(450.0 * 1.4 < DETAIL_MAX_TOKENS as f64);
    }

    #[test]
    fn area_and_expand_requests() {
        let r = area_request(InpaintAction::Remove, "magenta", "  ", "PNG".into(), 7);
        assert_eq!(r.prompt, "Line colour: magenta\nAction: remove\nStart with: Remove\nThe user's words: (none)");
        // The action's words the app puts in front are not the user's.
        assert!(area_request(InpaintAction::Remove, "magenta", "Remove ", "PNG".into(), 7).prompt.ends_with("(none)"));
        assert!(area_request(InpaintAction::Remove, "magenta", "Remove the lamp", "PNG".into(), 7).prompt.ends_with("words: the lamp"));
        assert_eq!((r.images.len(), r.system_prompt.as_str(), r.sampling.seed, r.max_tokens), (1, AREA_SYSTEM_PROMPT, 7, AREA_MAX_TOKENS));
        let r = area_request(InpaintAction::Add, "cyan", " un vaso di fiori ", "PNG".into(), 1);
        assert!(r.prompt.ends_with("Action: add\nStart with: Add\nThe user's words: un vaso di fiori"));
        let r = expand_request("left and right", "", "PNG".into(), 3);
        assert_eq!(r.prompt, "Growing sides: left and right\nThe user's wishes: (none)");
        assert!(r.system_prompt == EXPAND_SYSTEM_PROMPT && r.images.len() == 1);
        // The instruction around it is Kokon's: the text is only what goes in the area.
        assert!(AREA_SYSTEM_PROMPT.contains("Never mention the line") && EXPAND_SYSTEM_PROMPT.contains("Never mention borders"));
    }

    #[test]
    fn compose_prompt_makes_a_new_picture() {
        // "Keep everything else unchanged" turns a composition into an edit of image 1.
        assert!(!COMPOSE_SYSTEM_PROMPT.contains("unchanged"));
        assert!(COMPOSE_SYSTEM_PROMPT.contains("<image1>, <image2>") && COMPOSE_SYSTEM_PROMPT.contains("new image"));
        assert!(!COMPOSE_LABEL_PROMPT.contains("unchanged"));
        // Long scenes are allowed for, within the token cap (~1.4 tokens a word at most).
        assert!(COMPOSE_SYSTEM_PROMPT.contains("280") && T2I_SYSTEM_PROMPT.contains("250"));
        assert!(300.0 * 1.4 < MAX_TOKENS as f64);
    }

    #[test]
    fn cleaning() {
        assert_eq!(clean("  A watercolour fox.\n").as_deref(), Some("A watercolour fox."));
        assert_eq!(clean("Prompt: A fox.").as_deref(), Some("A fox."));
        assert_eq!(clean("\"A fox in snow.\"").as_deref(), Some("A fox in snow."));
        let sign = "A shop sign reading \"CAFÉ\" above the door, lit by \"warm\" lamps.";
        assert_eq!(clean(sign).as_deref(), Some(sign), "inner quotes are text to render");
        assert_eq!(clean("\"OPEN\" glows in red neon above a \"BAR\"").as_deref(), Some("\"OPEN\" glows in red neon above a \"BAR\""));
        assert_eq!(clean(" \n "), None);
        assert_eq!(clean("Prompt:"), None);
        assert_eq!(clean("Ünïcode first").as_deref(), Some("Ünïcode first"), "no slicing inside a character");
    }

    #[test]
    fn bfl_pup_selection_and_requests() {
        let cfg = AppConfig::default();
        // Selection is by the gateway model id.
        assert!(bfl_pup("kokon-klein"));
        assert!(bfl_pup("klein-fast"));
        assert!(!bfl_pup("kokon-full"));
        assert!(!bfl_pup("kokon-turbo"));
        assert!(!bfl_pup(""));

        // t2i: BFL prompt, canvas-prefixed request, BFL sampling, no images.
        let r = request(
            &cfg,
            "kokon-klein",
            ResolvedMode::Generate,
            "a red fox asleep in a suitcase",
            Vec::new(),
            Some((1360, 768)),
            7,
        );
        assert_eq!(r.system_prompt, BFL_T2I_SYSTEM_PROMPT);
        assert_eq!(
            r.prompt,
            "TARGET CANVAS: 1360:768\na red fox asleep in a suitcase"
        );
        assert_eq!(r.max_tokens, BFL_MAX_TOKENS);
        assert_eq!(r.sampling.temperature, 0.0);
        assert_eq!(r.sampling.presence_penalty, 1.5);
        assert_eq!(r.sampling.seed, 7);
        assert!(r.images.is_empty());

        // t2i without a known canvas keeps the request verbatim.
        let r = request(&cfg, "kokon-klein", ResolvedMode::Generate, "a fox", Vec::new(), None, 7);
        assert_eq!(r.prompt, "a fox");

        // Edit: BFL i2i prompt and scaffold, the reference travels with the request.
        let img: String = "a-reference".into();
        let r = request(
            &cfg,
            "kokon-klein",
            ResolvedMode::Edit,
            "make the fox white",
            vec![img.clone()],
            None,
            7,
        );
        assert_eq!(r.system_prompt, BFL_I2I_SYSTEM_PROMPT);
        assert_eq!(
            r.prompt,
            "Reference images follow in order (the first image, the second image, ...).\nUser request: make the fox white\nOutput the caption."
        );
        assert_eq!(r.images, vec![img.clone()]);
        // i2i runs without the presence penalty the t2i task gets.
        assert_eq!(r.sampling.presence_penalty, 0.0);
        let r2 = request(
            &cfg,
            "kokon-klein",
            ResolvedMode::Generate,
            "a fox",
            Vec::new(),
            None,
            7,
        );
        assert_eq!(r2.sampling.presence_penalty, 1.5);

        // The Qwen path is untouched: built-in prompt, verbatim request, its sampling.
        let r = request(
            &cfg,
            "kokon-full",
            ResolvedMode::Edit,
            "make the fox white",
            vec![img.clone()],
            None,
            7,
        );
        assert_eq!(r.system_prompt, EDIT_SYSTEM_PROMPT);
        assert_eq!(r.prompt, "make the fox white");
        assert_eq!(r.max_tokens, MAX_TOKENS);
        assert_eq!(r.sampling.temperature, 0.7);

        // A custom Settings prompt tunes the Qwen path only.
        let mut cfg = AppConfig::default();
        cfg.enhancer_edit_prompt = "My rules.".into();
        let r = request(
            &cfg,
            "kokon-full",
            ResolvedMode::Edit,
            "x",
            Vec::new(),
            None,
            7,
        );
        assert_eq!(r.system_prompt, "My rules.");
        let r = request(
            &cfg,
            "kokon-klein",
            ResolvedMode::Edit,
            "x",
            Vec::new(),
            None,
            7,
        );
        assert_eq!(r.system_prompt, BFL_I2I_SYSTEM_PROMPT, "BFL prompts ignore the Settings fields");
    }
}
