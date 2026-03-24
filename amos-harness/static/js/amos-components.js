/**
 * AMOS Component Library
 *
 * Pre-built, data-aware UI components for canvas iframes.
 * Components fetch from the /api/v1/data REST API and handle
 * rendering, CRUD, sorting, filtering, and pagination automatically.
 *
 * Usage (inside canvas JS):
 *   const table = new AMOS.DataTable(document.getElementById('app'), {
 *     collection: 'contacts',
 *     searchable: true,
 *     actions: ['edit', 'delete'],
 *   });
 */
(function () {
  'use strict';

  const AMOS = {};

  // ── Data helpers ─────────────────────────────────────────────────────

  /** Fetch records from the data API. Returns { records, total, limit, offset, schema }. */
  AMOS.fetchData = async function (collection, params = {}) {
    const url = new URL(`/api/v1/data/${encodeURIComponent(collection)}`, window.location.origin);
    if (params.filters) url.searchParams.set('filters', JSON.stringify(params.filters));
    if (params.sort_by) url.searchParams.set('sort_by', params.sort_by);
    if (params.sort_dir) url.searchParams.set('sort_dir', params.sort_dir);
    if (params.limit != null) url.searchParams.set('limit', String(params.limit));
    if (params.offset != null) url.searchParams.set('offset', String(params.offset));
    if (params.search) url.searchParams.set('search', params.search);
    const res = await fetch(url);
    if (!res.ok) throw new Error(`Data fetch failed: ${res.status}`);
    return res.json();
  };

  /** Fetch collection schema. Returns full collection object with fields. */
  AMOS.fetchSchema = async function (collection) {
    const res = await fetch(`/api/v1/data/${encodeURIComponent(collection)}/schema`);
    if (!res.ok) throw new Error(`Schema fetch failed: ${res.status}`);
    return res.json();
  };

  /** Create a record. Returns the created record. */
  AMOS.createRecord = async function (collection, data) {
    const res = await fetch(`/api/v1/data/${encodeURIComponent(collection)}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    });
    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      throw new Error(err.error || `Create failed: ${res.status}`);
    }
    return res.json();
  };

  /** Update a record. Returns the updated record. */
  AMOS.updateRecord = async function (collection, id, data) {
    const res = await fetch(`/api/v1/data/${encodeURIComponent(collection)}/${id}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    });
    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      throw new Error(err.error || `Update failed: ${res.status}`);
    }
    return res.json();
  };

  /** Delete a record. */
  AMOS.deleteRecord = async function (collection, id) {
    const res = await fetch(`/api/v1/data/${encodeURIComponent(collection)}/${id}`, {
      method: 'DELETE',
    });
    if (!res.ok) throw new Error(`Delete failed: ${res.status}`);
  };

  // ── Base Component ───────────────────────────────────────────────────

  class Component {
    constructor(el, opts = {}) {
      this.el = typeof el === 'string' ? document.querySelector(el) : el;
      this.opts = opts;
      this._listeners = {};
      this._filters = {};
      if (this.el) this.init();
    }

    /** Override in subclass. Called after constructor. */
    async init() {}

    /** Send a postMessage to the parent window (agent). */
    notify(action, data = {}) {
      window.parent.postMessage({ type: 'canvas-action', action, data }, '*');
    }

    /** Send a chat message back to the agent. */
    chat(message) {
      window.parent.postMessage({ type: 'canvas-chat', message }, '*');
    }

    /** Simple event emitter. */
    on(event, fn) {
      (this._listeners[event] = this._listeners[event] || []).push(fn);
    }
    emit(event, ...args) {
      (this._listeners[event] || []).forEach(fn => fn(...args));
    }

    /** Apply external filters and refresh. */
    setFilters(filters) {
      this._filters = { ...this._filters, ...filters };
      this.refresh();
    }

    /** Override in subclass to re-fetch and re-render. */
    async refresh() {}

    /** Utility: render Lucide icons in this component's DOM. */
    _icons() {
      if (typeof lucide !== 'undefined') lucide.createIcons({ attrs: {}, nameAttr: 'data-lucide' });
    }

    /** Utility: escape HTML to prevent XSS. */
    _esc(str) {
      const div = document.createElement('div');
      div.textContent = str == null ? '' : String(str);
      return div.innerHTML;
    }

    /** Utility: format a value for display. */
    _format(value, fmt) {
      if (value == null) return '—';
      switch (fmt) {
        case 'currency':
          return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(value);
        case 'percent':
          return new Intl.NumberFormat('en-US', { style: 'percent', maximumFractionDigits: 1 }).format(value / 100);
        case 'number':
          return new Intl.NumberFormat('en-US').format(value);
        default:
          return String(value);
      }
    }
  }

  AMOS.Component = Component;

  // ── 1. MetricCard ────────────────────────────────────────────────────

  /**
   * Single stat card.
   * Options:
   *   collection, label, aggregate (count|sum|avg|min|max),
   *   field, filters, format (number|currency|percent),
   *   icon, color
   */
  class MetricCard extends Component {
    async init() {
      this.el.classList.add('card', 'h-100');
      await this.refresh();
    }

    async refresh() {
      const { collection, label, aggregate = 'count', field, filters, format, icon, color = 'primary' } = this.opts;
      const merged = { ...filters, ...this._filters };
      try {
        const { records } = await AMOS.fetchData(collection, { filters: Object.keys(merged).length ? merged : undefined, limit: 500 });
        const value = this._aggregate(records, aggregate, field);
        this._render(value, label, icon, color, format);
      } catch (e) {
        this.el.innerHTML = `<div class="card-body text-danger">Error: ${this._esc(e.message)}</div>`;
      }
    }

    _aggregate(records, agg, field) {
      if (agg === 'count') return records.length;
      const vals = records.map(r => Number(r.data?.[field])).filter(v => !isNaN(v));
      if (!vals.length) return 0;
      switch (agg) {
        case 'sum': return vals.reduce((a, b) => a + b, 0);
        case 'avg': return vals.reduce((a, b) => a + b, 0) / vals.length;
        case 'min': return Math.min(...vals);
        case 'max': return Math.max(...vals);
        default: return vals.length;
      }
    }

    _render(value, label, icon, color, format) {
      const iconHtml = icon ? `<i data-lucide="${this._esc(icon)}" class="me-2" style="width:24px;height:24px"></i>` : '';
      this.el.innerHTML = `
        <div class="card-body">
          <div class="d-flex align-items-center mb-2">
            ${iconHtml}
            <span class="text-muted small">${this._esc(label || '')}</span>
          </div>
          <h2 class="mb-0 text-${this._esc(color)}">${this._format(value, format)}</h2>
        </div>`;
      this._icons();
    }
  }

  AMOS.MetricCard = MetricCard;

  // ── 2. DataTable ─────────────────────────────────────────────────────

  /**
   * Sortable, filterable, paginated table with CRUD.
   * Options:
   *   collection, columns (array of field names or {field, label, sortable}),
   *   actions (array: 'edit'|'delete'|'view'), searchable, sortable,
   *   pageSize (25), filters, createButton (bool|string label)
   */
  class DataTable extends Component {
    async init() {
      this._page = 0;
      this._sortBy = null;
      this._sortDir = 'desc';
      this._search = '';
      this._schema = null;
      this._debounce = null;
      await this.refresh();
    }

    async refresh() {
      const { collection, pageSize = 25, filters } = this.opts;
      const merged = { ...filters, ...this._filters };
      try {
        const params = {
          filters: Object.keys(merged).length ? merged : undefined,
          sort_by: this._sortBy,
          sort_dir: this._sortDir,
          limit: pageSize,
          offset: this._page * pageSize,
        };
        if (this._search) params.search = this._search;

        const data = await AMOS.fetchData(collection, params);
        this._schema = data.schema;
        this._total = data.total;
        this._records = data.records;
        this._render();
      } catch (e) {
        this.el.innerHTML = `<div class="alert alert-danger">Error loading data: ${this._esc(e.message)}</div>`;
      }
    }

    _getColumns() {
      if (this.opts.columns) {
        return this.opts.columns.map(c =>
          typeof c === 'string' ? { field: c, label: this._fieldLabel(c), sortable: true } : c
        );
      }
      // Auto-detect from schema
      if (this._schema?.fields) {
        return this._schema.fields.map(f => ({ field: f.name, label: f.display_name, sortable: true }));
      }
      return [];
    }

    _fieldLabel(name) {
      const f = this._schema?.fields?.find(f => f.name === name);
      return f ? f.display_name : name.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase());
    }

    _render() {
      const cols = this._getColumns();
      const { searchable, createButton, pageSize = 25, actions = [] } = this.opts;

      let html = '';

      // Toolbar
      const toolbarParts = [];
      if (searchable) {
        toolbarParts.push(`<input type="text" class="form-control form-control-sm" placeholder="Search\u2026" value="${this._esc(this._search)}" data-amos-search style="max-width:250px">`);
      }
      if (createButton) {
        const label = typeof createButton === 'string' ? createButton : 'New';
        toolbarParts.push(`<button class="btn btn-sm btn-primary" data-amos-create><i data-lucide="plus" style="width:14px;height:14px"></i> ${this._esc(label)}</button>`);
      }
      if (toolbarParts.length) {
        html += `<div class="d-flex justify-content-between align-items-center mb-3 gap-2">${toolbarParts.join('')}</div>`;
      }

      // Table
      html += '<div class="table-responsive"><table class="table table-hover table-sm align-middle">';
      html += '<thead><tr>';
      for (const col of cols) {
        const sortable = col.sortable !== false && this.opts.sortable !== false;
        if (sortable) {
          const arrow = this._sortBy === col.field ? (this._sortDir === 'asc' ? ' \u25B2' : ' \u25BC') : '';
          html += `<th role="button" data-amos-sort="${this._esc(col.field)}" class="user-select-none">${this._esc(col.label)}${arrow}</th>`;
        } else {
          html += `<th>${this._esc(col.label)}</th>`;
        }
      }
      if (actions.length) html += '<th class="text-end">Actions</th>';
      html += '</tr></thead><tbody>';

      for (const rec of this._records) {
        html += '<tr>';
        for (const col of cols) {
          const val = rec.data?.[col.field];
          html += `<td>${this._esc(val)}</td>`;
        }
        if (actions.length) {
          html += '<td class="text-end text-nowrap">';
          if (actions.includes('view')) {
            html += `<button class="btn btn-sm btn-outline-secondary me-1" data-amos-view="${rec.id}" title="View"><i data-lucide="eye" style="width:14px;height:14px"></i></button>`;
          }
          if (actions.includes('edit')) {
            html += `<button class="btn btn-sm btn-outline-primary me-1" data-amos-edit="${rec.id}" title="Edit"><i data-lucide="pencil" style="width:14px;height:14px"></i></button>`;
          }
          if (actions.includes('delete')) {
            html += `<button class="btn btn-sm btn-outline-danger" data-amos-delete="${rec.id}" title="Delete"><i data-lucide="trash-2" style="width:14px;height:14px"></i></button>`;
          }
          html += '</td>';
        }
        html += '</tr>';
      }

      if (!this._records.length) {
        html += `<tr><td colspan="${cols.length + (actions.length ? 1 : 0)}" class="text-center text-muted py-4">No records found</td></tr>`;
      }

      html += '</tbody></table></div>';

      // Pagination
      const totalPages = Math.ceil(this._total / pageSize);
      if (totalPages > 1) {
        html += '<nav><ul class="pagination pagination-sm justify-content-center">';
        html += `<li class="page-item ${this._page === 0 ? 'disabled' : ''}"><a class="page-link" href="#" data-amos-page="${this._page - 1}">Prev</a></li>`;
        for (let i = 0; i < totalPages && i < 10; i++) {
          html += `<li class="page-item ${i === this._page ? 'active' : ''}"><a class="page-link" href="#" data-amos-page="${i}">${i + 1}</a></li>`;
        }
        if (totalPages > 10) {
          html += `<li class="page-item disabled"><span class="page-link">\u2026</span></li>`;
        }
        html += `<li class="page-item ${this._page >= totalPages - 1 ? 'disabled' : ''}"><a class="page-link" href="#" data-amos-page="${this._page + 1}">Next</a></li>`;
        html += '</ul></nav>';
      }

      this.el.innerHTML = html;
      this._icons();
      this._bind();
    }

    _bind() {
      // Search
      const searchInput = this.el.querySelector('[data-amos-search]');
      if (searchInput) {
        searchInput.addEventListener('input', (e) => {
          clearTimeout(this._debounce);
          this._debounce = setTimeout(() => {
            this._search = e.target.value;
            this._page = 0;
            this.refresh();
          }, 300);
        });
      }

      // Sort
      this.el.querySelectorAll('[data-amos-sort]').forEach(th => {
        th.addEventListener('click', () => {
          const field = th.dataset.amosSort;
          if (this._sortBy === field) {
            this._sortDir = this._sortDir === 'asc' ? 'desc' : 'asc';
          } else {
            this._sortBy = field;
            this._sortDir = 'asc';
          }
          this.refresh();
        });
      });

      // Pagination
      this.el.querySelectorAll('[data-amos-page]').forEach(a => {
        a.addEventListener('click', (e) => {
          e.preventDefault();
          const page = parseInt(a.dataset.amosPage, 10);
          if (page >= 0 && page < Math.ceil(this._total / (this.opts.pageSize || 25))) {
            this._page = page;
            this.refresh();
          }
        });
      });

      // Delete
      this.el.querySelectorAll('[data-amos-delete]').forEach(btn => {
        btn.addEventListener('click', async () => {
          if (!confirm('Are you sure you want to delete this record?')) return;
          try {
            await AMOS.deleteRecord(this.opts.collection, btn.dataset.amosDelete);
            this.emit('delete', btn.dataset.amosDelete);
            this.refresh();
          } catch (e) {
            alert('Delete failed: ' + e.message);
          }
        });
      });

      // Edit
      this.el.querySelectorAll('[data-amos-edit]').forEach(btn => {
        btn.addEventListener('click', () => {
          this.emit('edit', btn.dataset.amosEdit);
          this.notify('edit-record', { collection: this.opts.collection, id: btn.dataset.amosEdit });
        });
      });

      // View
      this.el.querySelectorAll('[data-amos-view]').forEach(btn => {
        btn.addEventListener('click', () => {
          this.emit('view', btn.dataset.amosView);
          this.notify('view-record', { collection: this.opts.collection, id: btn.dataset.amosView });
        });
      });

      // Create
      const createBtn = this.el.querySelector('[data-amos-create]');
      if (createBtn) {
        createBtn.addEventListener('click', () => {
          this.emit('create');
          this.notify('create-record', { collection: this.opts.collection });
        });
      }
    }
  }

  AMOS.DataTable = DataTable;

  // ── 3. FormBuilder ───────────────────────────────────────────────────

  /**
   * Auto-generated form from collection schema.
   * Options:
   *   collection, recordId (for edit), fields (subset to show),
   *   layout ('vertical'|'horizontal'|'two-column'),
   *   onSubmit (callback), onCancel (callback)
   */
  class FormBuilder extends Component {
    async init() {
      this._schema = null;
      this._record = null;
      await this.refresh();
    }

    async refresh() {
      const { collection, recordId } = this.opts;
      try {
        this._schema = await AMOS.fetchSchema(collection);
        if (recordId) {
          const res = await fetch(`/api/v1/data/${encodeURIComponent(collection)}/${recordId}`);
          if (res.ok) this._record = await res.json();
        }
        this._render();
      } catch (e) {
        this.el.innerHTML = `<div class="alert alert-danger">Error: ${this._esc(e.message)}</div>`;
      }
    }

    _getFields() {
      const allFields = this._schema?.fields || [];
      if (this.opts.fields) {
        return allFields.filter(f => this.opts.fields.includes(f.name));
      }
      return allFields;
    }

    _render() {
      const fields = this._getFields();
      const layout = this.opts.layout || 'vertical';
      const isEdit = !!this._record;

      let html = `<form novalidate>`;

      const wrapClass = layout === 'two-column' ? 'row' : '';
      const colClass = layout === 'two-column' ? 'col-md-6' : '';

      if (wrapClass) html += `<div class="${wrapClass}">`;

      for (const field of fields) {
        const value = isEdit ? (this._record.data?.[field.name] ?? '') : (field.default_value ?? '');
        const required = field.required ? 'required' : '';
        const inputHtml = this._inputFor(field, value);

        if (layout === 'horizontal') {
          html += `<div class="row mb-3">
            <label class="col-sm-3 col-form-label" for="field-${this._esc(field.name)}">${this._esc(field.display_name)}${field.required ? ' <span class="text-danger">*</span>' : ''}</label>
            <div class="col-sm-9">${inputHtml}</div>
          </div>`;
        } else {
          html += `<div class="mb-3 ${colClass}">
            <label class="form-label" for="field-${this._esc(field.name)}">${this._esc(field.display_name)}${field.required ? ' <span class="text-danger">*</span>' : ''}</label>
            ${inputHtml}
            ${field.description ? `<div class="form-text">${this._esc(field.description)}</div>` : ''}
          </div>`;
        }
      }

      if (wrapClass) html += '</div>';

      html += `<div class="d-flex gap-2 mt-3">
        <button type="submit" class="btn btn-primary">${isEdit ? 'Update' : 'Create'}</button>
        <button type="button" class="btn btn-outline-secondary" data-amos-cancel>Cancel</button>
      </div>`;

      html += '</form>';

      this.el.innerHTML = html;
      this._bind();
    }

    _inputFor(field, value) {
      const id = `field-${field.name}`;
      const req = field.required ? 'required' : '';
      const esc = v => this._esc(v);

      switch (field.field_type) {
        case 'boolean':
          return `<div class="form-check">
            <input class="form-check-input" type="checkbox" id="${id}" name="${field.name}" ${value ? 'checked' : ''}>
          </div>`;

        case 'enum': {
          const choices = field.options?.choices || [];
          let opts = `<option value="">Select\u2026</option>`;
          for (const c of choices) {
            opts += `<option value="${esc(c)}" ${String(value) === String(c) ? 'selected' : ''}>${esc(c)}</option>`;
          }
          return `<select class="form-select" id="${id}" name="${field.name}" ${req}>${opts}</select>`;
        }

        case 'rich_text':
          return `<textarea class="form-control" id="${id}" name="${field.name}" rows="4" ${req}>${esc(value)}</textarea>`;

        case 'number':
          return `<input type="number" class="form-control" id="${id}" name="${field.name}" value="${esc(value)}" step="1" ${req}>`;

        case 'decimal':
          return `<input type="number" class="form-control" id="${id}" name="${field.name}" value="${esc(value)}" step="any" ${req}>`;

        case 'date':
          return `<input type="date" class="form-control" id="${id}" name="${field.name}" value="${esc(value)}" ${req}>`;

        case 'date_time':
          return `<input type="datetime-local" class="form-control" id="${id}" name="${field.name}" value="${esc(value)}" ${req}>`;

        case 'email':
          return `<input type="email" class="form-control" id="${id}" name="${field.name}" value="${esc(value)}" ${req}>`;

        case 'url':
          return `<input type="url" class="form-control" id="${id}" name="${field.name}" value="${esc(value)}" ${req}>`;

        case 'phone':
          return `<input type="tel" class="form-control" id="${id}" name="${field.name}" value="${esc(value)}" ${req}>`;

        case 'reference': {
          // For references, render as a text input for UUID — in the future this could be a select
          return `<input type="text" class="form-control" id="${id}" name="${field.name}" value="${esc(value)}" placeholder="UUID" ${req}>`;
        }

        case 'json':
          return `<textarea class="form-control font-monospace" id="${id}" name="${field.name}" rows="4" ${req}>${esc(typeof value === 'object' ? JSON.stringify(value, null, 2) : value)}</textarea>`;

        default: // text
          return `<input type="text" class="form-control" id="${id}" name="${field.name}" value="${esc(value)}" ${req}>`;
      }
    }

    _bind() {
      const form = this.el.querySelector('form');
      if (!form) return;

      form.addEventListener('submit', async (e) => {
        e.preventDefault();
        if (!form.checkValidity()) {
          form.classList.add('was-validated');
          return;
        }

        const data = {};
        const fields = this._getFields();
        for (const field of fields) {
          const input = form.querySelector(`[name="${field.name}"]`);
          if (!input) continue;

          if (field.field_type === 'boolean') {
            data[field.name] = input.checked;
          } else if (field.field_type === 'number') {
            data[field.name] = input.value === '' ? null : parseInt(input.value, 10);
          } else if (field.field_type === 'decimal') {
            data[field.name] = input.value === '' ? null : parseFloat(input.value);
          } else if (field.field_type === 'json') {
            try { data[field.name] = JSON.parse(input.value); } catch { data[field.name] = input.value; }
          } else {
            data[field.name] = input.value || null;
          }
        }

        try {
          const submitBtn = form.querySelector('[type="submit"]');
          submitBtn.disabled = true;
          submitBtn.innerHTML = '<span class="spinner-border spinner-border-sm"></span> Saving\u2026';

          let result;
          if (this.opts.recordId) {
            result = await AMOS.updateRecord(this.opts.collection, this.opts.recordId, data);
          } else {
            result = await AMOS.createRecord(this.opts.collection, data);
          }

          this.emit('submit', result);
          if (this.opts.onSubmit) this.opts.onSubmit(result);
          this.notify('record-saved', { collection: this.opts.collection, record: result });
        } catch (e) {
          alert('Save failed: ' + e.message);
        } finally {
          const submitBtn = form.querySelector('[type="submit"]');
          if (submitBtn) {
            submitBtn.disabled = false;
            submitBtn.textContent = this.opts.recordId ? 'Update' : 'Create';
          }
        }
      });

      const cancelBtn = this.el.querySelector('[data-amos-cancel]');
      if (cancelBtn) {
        cancelBtn.addEventListener('click', () => {
          this.emit('cancel');
          if (this.opts.onCancel) this.opts.onCancel();
          this.notify('form-cancelled', { collection: this.opts.collection });
        });
      }
    }
  }

  AMOS.FormBuilder = FormBuilder;

  // ── 4. Chart ─────────────────────────────────────────────────────────

  /**
   * Chart.js wrapper backed by collection data.
   * Options:
   *   collection, type ('bar'|'line'|'pie'|'doughnut'),
   *   labelField, valueField, aggregate ('count'|'sum'|'avg'),
   *   title, data (static override), colors, filters
   */
  class Chart extends Component {
    async init() {
      this._canvas = document.createElement('canvas');
      this.el.appendChild(this._canvas);
      this._chart = null;
      await this.refresh();
    }

    async refresh() {
      const { collection, type: chartType = 'bar', labelField, valueField, aggregate = 'count', title, data: staticData, colors, filters } = this.opts;

      let labels, values;

      if (staticData) {
        labels = staticData.labels || [];
        values = staticData.values || [];
      } else if (collection && labelField) {
        const merged = { ...filters, ...this._filters };
        try {
          const res = await AMOS.fetchData(collection, { filters: Object.keys(merged).length ? merged : undefined, limit: 500 });
          const grouped = {};
          for (const rec of res.records) {
            const key = String(rec.data?.[labelField] ?? 'Unknown');
            if (!grouped[key]) grouped[key] = [];
            grouped[key].push(rec);
          }
          labels = Object.keys(grouped);
          values = labels.map(key => {
            const recs = grouped[key];
            if (aggregate === 'count') return recs.length;
            const nums = recs.map(r => Number(r.data?.[valueField])).filter(v => !isNaN(v));
            if (!nums.length) return 0;
            if (aggregate === 'sum') return nums.reduce((a, b) => a + b, 0);
            if (aggregate === 'avg') return nums.reduce((a, b) => a + b, 0) / nums.length;
            return nums.length;
          });
        } catch (e) {
          this.el.innerHTML = `<div class="alert alert-danger">Chart error: ${this._esc(e.message)}</div>`;
          return;
        }
      } else {
        return;
      }

      if (typeof window.Chart === 'undefined') {
        this.el.innerHTML = '<div class="alert alert-warning">Chart.js not loaded</div>';
        return;
      }

      const defaultColors = [
        '#0d6efd', '#6610f2', '#6f42c1', '#d63384', '#dc3545',
        '#fd7e14', '#ffc107', '#198754', '#20c997', '#0dcaf0',
      ];
      const bgColors = colors || defaultColors.slice(0, labels.length);

      if (this._chart) this._chart.destroy();

      this._chart = new window.Chart(this._canvas.getContext('2d'), {
        type: chartType,
        data: {
          labels,
          datasets: [{
            label: title || '',
            data: values,
            backgroundColor: bgColors,
            borderColor: chartType === 'line' ? bgColors[0] : bgColors,
            borderWidth: chartType === 'line' ? 2 : 1,
            fill: chartType === 'line' ? false : undefined,
          }],
        },
        options: {
          responsive: true,
          maintainAspectRatio: false,
          plugins: {
            title: title ? { display: true, text: title } : { display: false },
            legend: { display: ['pie', 'doughnut'].includes(chartType) },
          },
          scales: ['pie', 'doughnut'].includes(chartType) ? {} : {
            y: { beginAtZero: true },
          },
        },
      });
    }
  }

  AMOS.Chart = Chart;

  // ── 5. KanbanBoard ───────────────────────────────────────────────────

  /**
   * Drag-and-drop kanban board grouped by an enum field.
   * Options:
   *   collection, groupBy, cardTitle, cardSubtitle, cardFields (extra fields to show),
   *   filters
   */
  class KanbanBoard extends Component {
    async init() {
      this._schema = null;
      this._records = [];
      await this.refresh();
    }

    async refresh() {
      const { collection, filters } = this.opts;
      const merged = { ...filters, ...this._filters };
      try {
        this._schema = await AMOS.fetchSchema(collection);
        const res = await AMOS.fetchData(collection, { filters: Object.keys(merged).length ? merged : undefined, limit: 500 });
        this._records = res.records;
        this._render();
      } catch (e) {
        this.el.innerHTML = `<div class="alert alert-danger">Error: ${this._esc(e.message)}</div>`;
      }
    }

    _getColumns() {
      const { groupBy } = this.opts;
      const field = this._schema?.fields?.find(f => f.name === groupBy);
      if (field?.options?.choices) return field.options.choices;
      // Derive from data
      const unique = [...new Set(this._records.map(r => r.data?.[groupBy]).filter(Boolean))];
      return unique.length ? unique : ['No Group'];
    }

    _render() {
      const { groupBy, cardTitle, cardSubtitle, cardFields = [] } = this.opts;
      const columns = this._getColumns();
      const colWidth = Math.max(250, Math.floor(100 / columns.length));

      let html = '<div class="d-flex gap-3 overflow-auto pb-3" style="min-height:400px">';

      for (const col of columns) {
        const cards = this._records.filter(r => String(r.data?.[groupBy]) === String(col));
        html += `<div class="kanban-column flex-shrink-0" data-column="${this._esc(col)}" style="min-width:${colWidth}px;width:${colWidth}px">`;
        html += `<div class="d-flex justify-content-between align-items-center mb-2">
          <h6 class="mb-0 fw-semibold">${this._esc(col)}</h6>
          <span class="badge bg-secondary">${cards.length}</span>
        </div>`;
        html += `<div class="kanban-cards d-flex flex-column gap-2" data-column="${this._esc(col)}" style="min-height:100px">`;

        for (const rec of cards) {
          html += `<div class="card card-body p-2 kanban-card" draggable="true" data-id="${rec.id}">`;
          if (cardTitle) html += `<div class="fw-semibold small">${this._esc(rec.data?.[cardTitle])}</div>`;
          if (cardSubtitle) html += `<div class="text-muted small">${this._esc(rec.data?.[cardSubtitle])}</div>`;
          for (const f of cardFields) {
            html += `<div class="text-muted small">${this._esc(rec.data?.[f])}</div>`;
          }
          html += '</div>';
        }

        html += '</div></div>';
      }

      html += '</div>';

      // Minimal kanban styling
      html += `<style>
        .kanban-column { background: var(--bs-light, #f8f9fa); border-radius: .5rem; padding: .75rem; }
        .kanban-card { cursor: grab; transition: box-shadow .15s; }
        .kanban-card:active { cursor: grabbing; }
        .kanban-card.dragging { opacity: .5; }
        .kanban-cards.drag-over { background: rgba(13,110,253,.05); border-radius: .25rem; }
      </style>`;

      this.el.innerHTML = html;
      this._bindDragDrop();
    }

    _bindDragDrop() {
      const cards = this.el.querySelectorAll('.kanban-card');
      const columns = this.el.querySelectorAll('.kanban-cards');

      cards.forEach(card => {
        card.addEventListener('dragstart', (e) => {
          card.classList.add('dragging');
          e.dataTransfer.setData('text/plain', card.dataset.id);
          e.dataTransfer.effectAllowed = 'move';
        });
        card.addEventListener('dragend', () => card.classList.remove('dragging'));
      });

      columns.forEach(col => {
        col.addEventListener('dragover', (e) => {
          e.preventDefault();
          e.dataTransfer.dropEffect = 'move';
          col.classList.add('drag-over');
        });
        col.addEventListener('dragleave', () => col.classList.remove('drag-over'));
        col.addEventListener('drop', async (e) => {
          e.preventDefault();
          col.classList.remove('drag-over');
          const recordId = e.dataTransfer.getData('text/plain');
          const newValue = col.dataset.column;
          try {
            await AMOS.updateRecord(this.opts.collection, recordId, { [this.opts.groupBy]: newValue });
            this.emit('move', { id: recordId, [this.opts.groupBy]: newValue });
            this.refresh();
          } catch (err) {
            alert('Move failed: ' + err.message);
          }
        });
      });
    }
  }

  AMOS.KanbanBoard = KanbanBoard;

  // ── 6. FilterBar ─────────────────────────────────────────────────────

  /**
   * Filter controls that drive other components.
   * Options:
   *   collection, fields (array of field names to show filters for),
   *   targets (array of Component instances to call setFilters/refresh on)
   */
  class FilterBar extends Component {
    async init() {
      this._schema = null;
      this._values = {};
      try {
        this._schema = await AMOS.fetchSchema(this.opts.collection);
        this._render();
      } catch (e) {
        this.el.innerHTML = `<div class="alert alert-warning">Filter error: ${this._esc(e.message)}</div>`;
      }
    }

    _getFields() {
      const allFields = this._schema?.fields || [];
      if (this.opts.fields) {
        return allFields.filter(f => this.opts.fields.includes(f.name));
      }
      // Default: show enum and boolean fields
      return allFields.filter(f => ['enum', 'boolean'].includes(f.field_type));
    }

    _render() {
      const fields = this._getFields();
      let html = '<div class="d-flex flex-wrap gap-2 align-items-end">';

      for (const field of fields) {
        html += `<div>`;
        html += `<label class="form-label small mb-1">${this._esc(field.display_name)}</label>`;

        if (field.field_type === 'enum') {
          const choices = field.options?.choices || [];
          html += `<select class="form-select form-select-sm" data-filter="${this._esc(field.name)}">`;
          html += `<option value="">All</option>`;
          for (const c of choices) {
            const sel = this._values[field.name] === c ? 'selected' : '';
            html += `<option value="${this._esc(c)}" ${sel}>${this._esc(c)}</option>`;
          }
          html += '</select>';
        } else if (field.field_type === 'boolean') {
          html += `<select class="form-select form-select-sm" data-filter="${this._esc(field.name)}">`;
          html += `<option value="">All</option>`;
          html += `<option value="true" ${this._values[field.name] === true ? 'selected' : ''}>Yes</option>`;
          html += `<option value="false" ${this._values[field.name] === false ? 'selected' : ''}>No</option>`;
          html += '</select>';
        } else if (field.field_type === 'date' || field.field_type === 'date_time') {
          html += `<input type="date" class="form-control form-control-sm" data-filter="${this._esc(field.name)}" value="${this._esc(this._values[field.name] || '')}">`;
        } else {
          html += `<input type="text" class="form-control form-control-sm" placeholder="${this._esc(field.display_name)}" data-filter="${this._esc(field.name)}" value="${this._esc(this._values[field.name] || '')}">`;
        }

        html += '</div>';
      }

      html += `<div><button class="btn btn-sm btn-outline-secondary" data-filter-clear>Clear</button></div>`;
      html += '</div>';

      this.el.innerHTML = html;
      this._bind();
    }

    _bind() {
      this.el.querySelectorAll('[data-filter]').forEach(el => {
        const event = el.tagName === 'SELECT' ? 'change' : 'input';
        el.addEventListener(event, () => {
          const field = el.dataset.filter;
          let value = el.value;

          if (value === '') {
            delete this._values[field];
          } else if (value === 'true') {
            this._values[field] = true;
          } else if (value === 'false') {
            this._values[field] = false;
          } else {
            this._values[field] = value;
          }

          this._applyFilters();
        });
      });

      const clearBtn = this.el.querySelector('[data-filter-clear]');
      if (clearBtn) {
        clearBtn.addEventListener('click', () => {
          this._values = {};
          this._render();
          this._applyFilters();
        });
      }
    }

    _applyFilters() {
      const filters = { ...this._values };
      const targets = this.opts.targets || [];
      for (const target of targets) {
        if (typeof target.setFilters === 'function') {
          target.setFilters(filters);
        }
      }
      this.emit('change', filters);
    }
  }

  AMOS.FilterBar = FilterBar;

  // ── Export ────────────────────────────────────────────────────────────

  window.AMOS = AMOS;
})();
