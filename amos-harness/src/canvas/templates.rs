//! Built-in canvas templates
//!
//! Static fallback templates when AI generation is not available or fails

use super::types::{CanvasTemplate, CanvasType};
use uuid::Uuid;

/// Get a default template by key
pub fn get_template(key: &str) -> Option<CanvasTemplate> {
    match key {
        "list" => Some(list_template()),
        "kanban" => Some(kanban_template()),
        "form" => Some(form_template()),
        "detail" => Some(detail_template()),
        "dashboard" => Some(dashboard_template()),
        "calendar" => Some(calendar_template()),
        "freeform" => Some(freeform_template()),
        _ => None,
    }
}

/// List view template
pub fn list_template() -> CanvasTemplate {
    CanvasTemplate {
        id: Uuid::nil(),
        key: "list".to_string(),
        name: "List View".to_string(),
        canvas_type: CanvasType::Dynamic,
        html_content: Some(r#"
<div class="container-fluid p-4">
    <div class="d-flex justify-content-between align-items-center mb-4">
        <h2>{{ title }}</h2>
        <button class="btn btn-primary" data-action="create">
            <i data-lucide="plus"></i>
            Create New
        </button>
    </div>

    <div class="card">
        <div class="card-body">
            <table class="table table-hover">
                <thead>
                    <tr>
                        {% for column in columns %}
                        <th>{{ column }}</th>
                        {% endfor %}
                        <th class="text-end">Actions</th>
                    </tr>
                </thead>
                <tbody>
                    {% for item in items %}
                    <tr>
                        {% for column in columns %}
                        <td>{{ item[column] }}</td>
                        {% endfor %}
                        <td class="text-end">
                            <button class="btn btn-sm btn-outline-primary" data-action="view" data-id="{{ item.id }}">
                                <i data-lucide="eye"></i>
                            </button>
                            <button class="btn btn-sm btn-outline-secondary" data-action="edit" data-id="{{ item.id }}">
                                <i data-lucide="edit"></i>
                            </button>
                            <button class="btn btn-sm btn-outline-danger" data-action="delete" data-id="{{ item.id }}">
                                <i data-lucide="trash-2"></i>
                            </button>
                        </td>
                    </tr>
                    {% endfor %}
                </tbody>
            </table>
        </div>
    </div>
</div>
"#.to_string()),
        js_content: Some(r#"
// Initialize Lucide icons
if (typeof lucide !== 'undefined') {
    lucide.createIcons();
}

// Handle action buttons
document.addEventListener('click', (e) => {
    const button = e.target.closest('[data-action]');
    if (button) {
        const action = button.dataset.action;
        const id = button.dataset.id;
        window.parent.postMessage({ type: 'canvas-action', action, id }, '*');
    }
});
"#.to_string()),
        css_content: Some(r#"
.table {
    margin-bottom: 0;
}

.btn-sm {
    margin: 0 2px;
}
"#.to_string()),
        metadata: None,
        version: 1,
        active: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

/// Kanban board template
pub fn kanban_template() -> CanvasTemplate {
    CanvasTemplate {
        id: Uuid::nil(),
        key: "kanban".to_string(),
        name: "Kanban Board".to_string(),
        canvas_type: CanvasType::Kanban,
        html_content: Some(
            r#"
<div class="container-fluid p-4">
    <h2 class="mb-4">{{ title }}</h2>

    <div class="kanban-board">
        {% for column in columns %}
        <div class="kanban-column" data-column="{{ column.key }}">
            <div class="kanban-header">
                <h5>{{ column.name }}</h5>
                <span class="badge bg-secondary">{{ column.items | length }}</span>
            </div>
            <div class="kanban-items">
                {% for item in column.items %}
                <div class="kanban-card" data-id="{{ item.id }}" draggable="true">
                    <h6>{{ item.title }}</h6>
                    <p class="text-muted small">{{ item.description }}</p>
                    {% if item.labels %}
                    <div class="labels">
                        {% for label in item.labels %}
                        <span class="badge badge-sm bg-{{ label.color }}">{{ label.name }}</span>
                        {% endfor %}
                    </div>
                    {% endif %}
                </div>
                {% endfor %}
            </div>
        </div>
        {% endfor %}
    </div>
</div>
"#
            .to_string(),
        ),
        js_content: Some(
            r#"
// Initialize Lucide icons
if (typeof lucide !== 'undefined') {
    lucide.createIcons();
}

// Drag and drop functionality
let draggedElement = null;

document.addEventListener('dragstart', (e) => {
    if (e.target.classList.contains('kanban-card')) {
        draggedElement = e.target;
        e.target.classList.add('dragging');
    }
});

document.addEventListener('dragend', (e) => {
    if (e.target.classList.contains('kanban-card')) {
        e.target.classList.remove('dragging');
    }
});

document.querySelectorAll('.kanban-items').forEach(column => {
    column.addEventListener('dragover', (e) => {
        e.preventDefault();
        const afterElement = getDragAfterElement(column, e.clientY);
        if (afterElement == null) {
            column.appendChild(draggedElement);
        } else {
            column.insertBefore(draggedElement, afterElement);
        }
    });

    column.addEventListener('drop', (e) => {
        e.preventDefault();
        const columnKey = e.target.closest('.kanban-column').dataset.column;
        const itemId = draggedElement.dataset.id;
        window.parent.postMessage({
            type: 'canvas-action',
            action: 'move',
            id: itemId,
            column: columnKey
        }, '*');
    });
});

function getDragAfterElement(container, y) {
    const draggableElements = [...container.querySelectorAll('.kanban-card:not(.dragging)')];

    return draggableElements.reduce((closest, child) => {
        const box = child.getBoundingClientRect();
        const offset = y - box.top - box.height / 2;
        if (offset < 0 && offset > closest.offset) {
            return { offset: offset, element: child };
        } else {
            return closest;
        }
    }, { offset: Number.NEGATIVE_INFINITY }).element;
}
"#
            .to_string(),
        ),
        css_content: Some(
            r#"
.kanban-board {
    display: flex;
    gap: 1rem;
    overflow-x: auto;
    padding-bottom: 1rem;
}

.kanban-column {
    flex: 0 0 300px;
    background: #f8f9fa;
    border-radius: 8px;
    padding: 1rem;
}

.kanban-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
}

.kanban-items {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    min-height: 200px;
}

.kanban-card {
    background: white;
    border-radius: 6px;
    padding: 1rem;
    box-shadow: 0 1px 3px rgba(0,0,0,0.1);
    cursor: move;
    transition: box-shadow 0.2s;
}

.kanban-card:hover {
    box-shadow: 0 4px 6px rgba(0,0,0,0.15);
}

.kanban-card.dragging {
    opacity: 0.5;
}

.labels {
    display: flex;
    gap: 0.25rem;
    margin-top: 0.5rem;
}
"#
            .to_string(),
        ),
        metadata: None,
        version: 1,
        active: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

/// Form template
pub fn form_template() -> CanvasTemplate {
    CanvasTemplate {
        id: Uuid::nil(),
        key: "form".to_string(),
        name: "Form".to_string(),
        canvas_type: CanvasType::Form,
        html_content: Some(r#"
<div class="container p-4">
    <div class="row justify-content-center">
        <div class="col-md-8">
            <div class="card">
                <div class="card-body">
                    <h3 class="mb-4">{{ title }}</h3>
                    <form id="canvasForm">
                        {% for field in fields %}
                        <div class="mb-3">
                            <label for="{{ field.key }}" class="form-label">
                                {{ field.label }}
                                {% if field.required %}<span class="text-danger">*</span>{% endif %}
                            </label>

                            {% if field.type == "text" or field.type == "email" or field.type == "number" %}
                            <input
                                type="{{ field.type }}"
                                class="form-control"
                                id="{{ field.key }}"
                                name="{{ field.key }}"
                                placeholder="{{ field.placeholder }}"
                                {% if field.required %}required{% endif %}
                            >
                            {% elif field.type == "textarea" %}
                            <textarea
                                class="form-control"
                                id="{{ field.key }}"
                                name="{{ field.key }}"
                                rows="4"
                                placeholder="{{ field.placeholder }}"
                                {% if field.required %}required{% endif %}
                            ></textarea>
                            {% elif field.type == "select" %}
                            <select
                                class="form-select"
                                id="{{ field.key }}"
                                name="{{ field.key }}"
                                {% if field.required %}required{% endif %}
                            >
                                <option value="">Choose...</option>
                                {% for option in field.options %}
                                <option value="{{ option.value }}">{{ option.label }}</option>
                                {% endfor %}
                            </select>
                            {% endif %}

                            {% if field.help %}
                            <div class="form-text">{{ field.help }}</div>
                            {% endif %}
                        </div>
                        {% endfor %}

                        <div class="d-flex justify-content-end gap-2">
                            <button type="button" class="btn btn-secondary" data-action="cancel">Cancel</button>
                            <button type="submit" class="btn btn-primary">Submit</button>
                        </div>
                    </form>
                </div>
            </div>
        </div>
    </div>
</div>
"#.to_string()),
        js_content: Some(r#"
document.getElementById('canvasForm').addEventListener('submit', (e) => {
    e.preventDefault();
    const formData = new FormData(e.target);
    const data = Object.fromEntries(formData.entries());
    window.parent.postMessage({ type: 'canvas-action', action: 'submit', data }, '*');
});

document.querySelector('[data-action="cancel"]')?.addEventListener('click', () => {
    window.parent.postMessage({ type: 'canvas-action', action: 'cancel' }, '*');
});
"#.to_string()),
        css_content: None,
        metadata: None,
        version: 1,
        active: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

/// Detail view template
pub fn detail_template() -> CanvasTemplate {
    CanvasTemplate {
        id: Uuid::nil(),
        key: "detail".to_string(),
        name: "Detail View".to_string(),
        canvas_type: CanvasType::Detail,
        html_content: Some(
            r#"
<div class="container p-4">
    <div class="d-flex justify-content-between align-items-center mb-4">
        <h2>{{ title }}</h2>
        <div class="btn-group">
            <button class="btn btn-outline-primary" data-action="edit">
                <i data-lucide="edit"></i> Edit
            </button>
            <button class="btn btn-outline-danger" data-action="delete">
                <i data-lucide="trash-2"></i> Delete
            </button>
        </div>
    </div>

    <div class="card">
        <div class="card-body">
            {% for section in sections %}
            <h5 class="mb-3">{{ section.title }}</h5>
            <div class="row mb-4">
                {% for field in section.fields %}
                <div class="col-md-6 mb-3">
                    <label class="text-muted small">{{ field.label }}</label>
                    <div class="fw-medium">{{ field.value }}</div>
                </div>
                {% endfor %}
            </div>
            {% endfor %}
        </div>
    </div>
</div>
"#
            .to_string(),
        ),
        js_content: Some(
            r#"
if (typeof lucide !== 'undefined') {
    lucide.createIcons();
}

document.addEventListener('click', (e) => {
    const button = e.target.closest('[data-action]');
    if (button) {
        const action = button.dataset.action;
        window.parent.postMessage({ type: 'canvas-action', action }, '*');
    }
});
"#
            .to_string(),
        ),
        css_content: None,
        metadata: None,
        version: 1,
        active: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

/// Dashboard template
pub fn dashboard_template() -> CanvasTemplate {
    CanvasTemplate {
        id: Uuid::nil(),
        key: "dashboard".to_string(),
        name: "Dashboard".to_string(),
        canvas_type: CanvasType::Dashboard,
        html_content: Some(
            r#"
<div class="container-fluid p-4">
    <h2 class="mb-4">{{ title }}</h2>

    <div class="row mb-4">
        {% for metric in metrics %}
        <div class="col-md-3">
            <div class="card">
                <div class="card-body">
                    <div class="d-flex justify-content-between align-items-center">
                        <div>
                            <p class="text-muted mb-1">{{ metric.label }}</p>
                            <h3 class="mb-0">{{ metric.value }}</h3>
                        </div>
                        <div class="metric-icon text-{{ metric.color }}">
                            <i data-lucide="{{ metric.icon }}"></i>
                        </div>
                    </div>
                    {% if metric.change %}
                    <small class="text-{{ metric.change_type }}">
                        {{ metric.change }} from last period
                    </small>
                    {% endif %}
                </div>
            </div>
        </div>
        {% endfor %}
    </div>

    <div class="row">
        {% for widget in widgets %}
        <div class="col-md-{{ widget.width | default(value=6) }}">
            <div class="card mb-4">
                <div class="card-body">
                    <h5 class="card-title">{{ widget.title }}</h5>
                    <div id="widget-{{ widget.key }}"></div>
                </div>
            </div>
        </div>
        {% endfor %}
    </div>
</div>
"#
            .to_string(),
        ),
        js_content: Some(
            r#"
if (typeof lucide !== 'undefined') {
    lucide.createIcons();
}
"#
            .to_string(),
        ),
        css_content: Some(
            r#"
.metric-icon {
    font-size: 2rem;
    opacity: 0.5;
}

.card {
    border: none;
    box-shadow: 0 2px 4px rgba(0,0,0,0.1);
}
"#
            .to_string(),
        ),
        metadata: None,
        version: 1,
        active: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

/// Calendar template
pub fn calendar_template() -> CanvasTemplate {
    CanvasTemplate {
        id: Uuid::nil(),
        key: "calendar".to_string(),
        name: "Calendar".to_string(),
        canvas_type: CanvasType::Calendar,
        html_content: Some(
            r#"
<div class="container-fluid p-4">
    <h2 class="mb-4">{{ title }}</h2>
    <div id="calendar"></div>
</div>
"#
            .to_string(),
        ),
        js_content: Some(
            r#"
// Placeholder for calendar initialization
// In production, would use FullCalendar or similar library
console.log('Calendar view initialized');
"#
            .to_string(),
        ),
        css_content: None,
        metadata: None,
        version: 1,
        active: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

/// Freeform template (blank canvas)
pub fn freeform_template() -> CanvasTemplate {
    CanvasTemplate {
        id: Uuid::nil(),
        key: "freeform".to_string(),
        name: "Freeform Canvas".to_string(),
        canvas_type: CanvasType::Freeform,
        html_content: Some(
            r#"
<div class="container p-4">
    <h2>Custom Canvas</h2>
    <p>This is a blank freeform canvas. Add your custom HTML, CSS, and JavaScript here.</p>
</div>
"#
            .to_string(),
        ),
        js_content: Some("// Add your custom JavaScript here".to_string()),
        css_content: Some("/* Add your custom CSS here */".to_string()),
        metadata: None,
        version: 1,
        active: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}
