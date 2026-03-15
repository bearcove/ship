use sailfish::TemplateOnce;

#[derive(TemplateOnce)]
#[template(path = "captain_bootstrap.stpl")]
pub struct CaptainBootstrapPrompt;

#[derive(TemplateOnce)]
#[template(path = "captain_resume.stpl")]
pub struct CaptainResumePrompt;

#[derive(TemplateOnce)]
#[template(path = "admiral_bootstrap.stpl")]
pub struct AdmiralBootstrapPrompt;

#[derive(TemplateOnce)]
#[template(path = "mate_task_preamble.stpl")]
pub struct MateTaskPreamble<'a> {
    pub work_instructions: &'a str,
    pub description: &'a str,
    pub file_section: &'a str,
}

#[derive(TemplateOnce)]
#[template(path = "summarizer_persona.stpl")]
pub struct SummarizerPersonaPrompt;
