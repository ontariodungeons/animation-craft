// Skills registry UI: knowledge-discovery info blocks for tech skills/tools.
// Fetches skills from the engine (`skills.list`) and renders one info block per skill.
// Each block is a tracked box (stable id + data-box-id) so the TUI/bridge sees it.

async function renderSkills() {
  const list = $("skills-list");
  if (!list) return;
  list.replaceChildren();
  let skills;
  try {
    const result = await invoke("engine_command", { id: "skills.list", params: {} });
    skills = result.skills || [];
  } catch (e) {
    list.append(el("p", { class: "why" }, `Could not load skills: ${String(e).slice(0, 200)}`));
    return;
  }
  if (!skills.length) {
    list.append(el("p", { class: "why" }, "No skills registered yet. Add one below."));
    return;
  }
  for (const skill of skills) {
    list.append(skillCard(skill));
  }
}

function statusDot(status) {
  const tone = status === "implemented" ? "ready" : status === "researching" ? "needs-attention" : "idle";
  const label = status === "implemented" ? "Implemented" : status === "researching" ? "Researching" : "Not implemented";
  return el("span", { class: "skill-status", title: label }, label);
}

function skillCard(skill) {
  const id = skill.id || "skill";
  const boxId = `skill-${String(id).replaceAll(/[^a-z0-9-]/gi, "-")}`;
  const implementedAt = (skill.implemented_at || []).join(", ") || "—";
  const implementedIn = (skill.implemented_in || []).join(", ") || "—";
  const head = el("div", { class: "panel-head" },
    el("span", { class: "title" }, skill.name),
    el("span", { class: "meta" }, skill.id),
    statusDot(skill.status));
  const body = el("div", { class: "skill-body" },
    skill.description ? el("p", { class: "skill-desc" }, skill.description) : null,
    el("div", { class: "skill-field" },
      el("strong", {}, "Source"),
      el("span", { class: "skill-value" }, skill.source || "—"),
      skill.source ? el("a", { class: "ghost", href: skill.source, target: "_blank", rel: "noopener", title: "Open docs" }, "Open docs") : null),
    el("div", { class: "skill-field" },
      el("strong", {}, "Implemented at (commands)"),
      el("span", { class: "skill-value" }, implementedAt)),
    el("div", { class: "skill-field" },
      el("strong", {}, "Implemented in (files)"),
      el("span", { class: "skill-value" }, implementedIn)),
    skill.integration_notes ? el("div", { class: "skill-field" },
      el("strong", {}, "Integration notes"),
      el("p", { class: "skill-value" }, skill.integration_notes)) : null,
    el("div", { class: "skill-actions" },
      el("button", { class: "ghost", onclick: () => markImplemented(skill.id), title: "Mark implemented" }, "Mark implemented"),
      el("button", { class: "ghost", onclick: () => openSkillDocs(skill), title: "Open source" }, "Open source")));
  return el("section", { class: "skill-card", id: boxId, "data-box-id": boxId, box: boxId },
    head, body);
}

function openSkillDocs(skill) {
  if (skill.source) openPath(skill.source);
  else toast("No source URL for this skill");
}

async function markImplemented(id) {
  const result = await invoke("engine_command", { id: "skills.info", params: { id } });
  const skill = result.skill;
  if (!skill) { toast("Skill not found"); return; }
  try {
    await invoke("engine_command", { id: "skills.add", params: {
      id: skill.id, name: skill.name, description: skill.description, status: "implemented",
      source: skill.source, implemented_at: skill.implemented_at, implemented_in: skill.implemented_in,
      integration_notes: skill.integration_notes,
    } });
    renderSkills();
    toast("Marked implemented");
  } catch (e) { toast(String(e)); }
}

function setupSkillsAdd() {
  const form = $("skills-add-form");
  const cancel = $("skills-add-cancel");
  const addSection = $("skills-add");
  if (!form) return;
  cancel.addEventListener("click", () => { addSection.hidden = true; form.reset(); });
  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    try {
      await invoke("engine_command", { id: "skills.add", params: {
        id: $("skill-id").value.trim(),
        name: $("skill-name").value.trim(),
        description: $("skill-description").value.trim(),
        status: $("skill-status").value,
        source: $("skill-source").value.trim(),
        implemented_at: $("skill-implemented-at").value.split(",").map((s) => s.trim()).filter(Boolean),
        implemented_in: $("skill-implemented-in").value.split(",").map((s) => s.trim()).filter(Boolean),
        integration_notes: $("skill-integration-notes").value.trim(),
      } });
      form.reset();
      addSection.hidden = true;
      renderSkills();
      toast("Skill registered");
    } catch (err) { toast(String(err)); }
  });
}
