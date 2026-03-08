-- Seed default canvas templates
INSERT INTO canvas_templates (key, name, canvas_type, html_content, js_content, css_content, metadata) VALUES

('dashboard', 'Dashboard', 'dashboard',
'<div class="dashboard-container">
  <div class="row g-4 mb-4" id="statsRow"></div>
  <div class="row g-4">
    <div class="col-lg-8"><div class="card"><div class="card-body" id="mainChart"><canvas id="chartCanvas"></canvas></div></div></div>
    <div class="col-lg-4"><div class="card"><div class="card-body" id="sidebar"><h5>Recent Activity</h5><div id="activityList"></div></div></div></div>
  </div>
</div>',
'setTimeout(function() {
  // Dashboard initialization
  const statsRow = document.getElementById("statsRow");
  if (statsRow && window.canvasData && window.canvasData.stats) {
    window.canvasData.stats.forEach(function(stat) {
      statsRow.innerHTML += ''<div class="col-sm-6 col-xl-3"><div class="card"><div class="card-body"><h6 class="text-muted">'' + stat.label + ''</h6><h3>'' + stat.value + ''</h3></div></div></div>'';
    });
  }
}, 100);',
'.dashboard-container { padding: 1rem; } .card { border-radius: 12px; box-shadow: 0 2px 8px rgba(0,0,0,0.05); }',
'{"description": "General-purpose dashboard with stats cards and charts"}'),

('data_list', 'Data List', 'data_grid',
'<div class="list-container">
  <div class="d-flex justify-content-between align-items-center mb-3">
    <h4 id="listTitle">Items</h4>
    <div class="d-flex gap-2">
      <input type="text" class="form-control form-control-sm" placeholder="Search..." id="searchInput" style="width:250px;">
      <button class="btn btn-primary btn-sm" id="addBtn"><i data-lucide="plus"></i> Add</button>
    </div>
  </div>
  <div class="table-responsive"><table class="table table-hover" id="dataTable"><thead id="tableHead"></thead><tbody id="tableBody"></tbody></table></div>
</div>',
'setTimeout(function() {
  var searchInput = document.getElementById("searchInput");
  if (searchInput) {
    searchInput.addEventListener("input", function() {
      var filter = this.value.toLowerCase();
      var rows = document.querySelectorAll("#tableBody tr");
      rows.forEach(function(row) { row.style.display = row.textContent.toLowerCase().includes(filter) ? "" : "none"; });
    });
  }
}, 100);',
'.list-container { padding: 1rem; } .table th { font-weight: 600; font-size: 0.85rem; text-transform: uppercase; color: #6c757d; }',
'{"description": "Searchable, sortable data table view"}'),

('kanban', 'Kanban Board', 'kanban',
'<div class="kanban-container">
  <div class="kanban-board d-flex gap-3 overflow-auto pb-3" id="kanbanBoard"></div>
</div>',
'setTimeout(function() {
  var board = document.getElementById("kanbanBoard");
  if (board && window.canvasData && window.canvasData.columns) {
    window.canvasData.columns.forEach(function(col) {
      var column = document.createElement("div");
      column.className = "kanban-column";
      column.innerHTML = ''<div class="kanban-header"><h6>'' + col.name + ''</h6><span class="badge bg-secondary">'' + (col.items ? col.items.length : 0) + ''</span></div><div class="kanban-items" data-status="'' + col.id + ''"></div>'';
      if (col.items) {
        var itemsDiv = column.querySelector(".kanban-items");
        col.items.forEach(function(item) {
          itemsDiv.innerHTML += ''<div class="kanban-card card mb-2"><div class="card-body p-2"><p class="mb-1">'' + item.title + ''</p></div></div>'';
        });
      }
      board.appendChild(column);
    });
  }
}, 100);',
'.kanban-column { min-width: 280px; max-width: 320px; background: #f8f9fa; border-radius: 8px; padding: 0.75rem; } .kanban-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.75rem; } .kanban-card { cursor: grab; transition: box-shadow 0.2s; } .kanban-card:hover { box-shadow: 0 2px 8px rgba(0,0,0,0.1); }',
'{"description": "Drag-and-drop Kanban board"}'),

('freeform', 'Freeform Canvas', 'freeform_canvas',
'<div class="freeform-container"><div id="freeformContent"></div></div>',
'// Freeform canvas - AMOS populates this with dynamic content',
'.freeform-container { padding: 1rem; min-height: 400px; }',
'{"description": "Free-form canvas for AI-generated content"}')

ON CONFLICT (key) DO UPDATE SET
  html_content = EXCLUDED.html_content,
  js_content = EXCLUDED.js_content,
  css_content = EXCLUDED.css_content,
  metadata = EXCLUDED.metadata,
  version = canvas_templates.version + 1,
  updated_at = NOW();
